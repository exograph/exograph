use async_graphql_value::ConstValue;
use async_graphql_value::Number;
use base64::DecodeError;
use chrono::prelude::*;
use chrono::DateTime;
use maybe_owned::MaybeOwned;
use payas_sql::database_error::DatabaseError;
use payas_sql::{
    array_util::{self, ArrayEntry},
    Column, FloatBits, IntBits, PhysicalColumn, PhysicalColumnType, SQLBytes, SQLParam,
};
use pg_bigdecimal::{BigDecimal, PgNumeric};

use std::str::FromStr;

use thiserror::Error;

use super::postgres_execution_error::PostgresExecutionError;

const NAIVE_DATE_FORMAT: &str = "%Y-%m-%d";
const NAIVE_TIME_FORMAT: &str = "%H:%M:%S%.f";

#[derive(Debug, Error)]
pub enum CastError {
    #[error("{0}")]
    Generic(String),

    #[error("{0}")]
    Date(String, #[source] chrono::format::ParseError),

    #[error("{0}")]
    Blob(#[from] DecodeError),

    #[error("{0}")]
    Uuid(#[from] uuid::Error),

    #[error("{0}")]
    BigDecimal(String),

    #[error("{0}")]
    Postgres(#[from] payas_sql::database_error::DatabaseError),
}

pub(crate) fn literal_column<'a>(
    value: &ConstValue,
    associated_column: &PhysicalColumn,
) -> Result<Column<'a>, PostgresExecutionError> {
    cast_value(value, &associated_column.typ)
        .map(|value| {
            value
                .map(|v| Column::Literal(MaybeOwned::Owned(v)))
                .unwrap_or(Column::Null)
        })
        .map_err(PostgresExecutionError::CastError)
}

pub(crate) fn cast_value(
    value: &ConstValue,
    destination_type: &PhysicalColumnType,
) -> Result<Option<Box<dyn SQLParam>>, CastError> {
    match value {
        ConstValue::Number(number) => cast_number(number, destination_type).map(Some),
        ConstValue::String(v) => cast_string(v, destination_type).map(Some),
        ConstValue::Boolean(v) => Ok(Some(Box::new(*v))),
        ConstValue::Null => Ok(None),
        ConstValue::Enum(v) => Ok(Some(Box::new(v.to_string()))), // We might need guidance from the database to do a correct translation
        ConstValue::List(elems) => cast_list(elems, destination_type),
        ConstValue::Object(_) => Ok(Some(cast_object(value, destination_type))),
        ConstValue::Binary(bytes) => Ok(Some(Box::new(SQLBytes(bytes.clone())))),
    }
}

fn cast_list(
    elems: &[ConstValue],
    destination_type: &PhysicalColumnType,
) -> Result<Option<Box<dyn SQLParam>>, CastError> {
    fn array_entry(elem: &ConstValue) -> ArrayEntry<ConstValue> {
        match elem {
            ConstValue::List(elems) => ArrayEntry::List(elems),
            _ => ArrayEntry::Single(elem),
        }
    }

    fn cast_value_with_error(
        value: &ConstValue,
        destination_type: &PhysicalColumnType,
    ) -> Result<Option<Box<dyn SQLParam>>, DatabaseError> {
        cast_value(value, destination_type).map_err(|error| DatabaseError::BoxedError(error.into()))
    }

    array_util::to_sql_param(elems, destination_type, array_entry, cast_value_with_error)
        .map_err(CastError::Postgres)
}

fn cast_number(
    number: &Number,
    destination_type: &PhysicalColumnType,
) -> Result<Box<dyn SQLParam>, CastError> {
    let result: Box<dyn SQLParam> = match destination_type {
        PhysicalColumnType::Int { bits } => match bits {
            IntBits::_16 => Box::new(number.as_i64().unwrap() as i16),
            IntBits::_32 => Box::new(number.as_i64().unwrap() as i32),
            IntBits::_64 => Box::new(number.as_i64().unwrap() as i64),
        },
        PhysicalColumnType::Float { bits } => match bits {
            FloatBits::_24 => Box::new(number.as_f64().unwrap() as f32),
            FloatBits::_53 => Box::new(number.as_f64().unwrap() as f64),
        },
        PhysicalColumnType::Numeric { .. } => {
            return Err(CastError::Generic(
                "Number literals cannot be specified for decimal fields".into(),
            ));
        }
        PhysicalColumnType::ColumnReference { ref_pk_type, .. } => {
            // TODO assumes that `id` columns are always integers
            cast_number(number, ref_pk_type)?
        }
        // TODO: Expand for other number types such as float
        _ => {
            return Err(CastError::Generic(
                "Unexpected destination_type for number value".into(),
            ));
        }
    };

    Ok(result)
}

fn cast_string(
    string: &str,
    destination_type: &PhysicalColumnType,
) -> Result<Box<dyn SQLParam>, CastError> {
    let value: Box<dyn SQLParam> =
        match destination_type {
            PhysicalColumnType::Numeric { .. } => {
                let decimal = match string {
                    "NaN" => PgNumeric { n: None },
                    _ => PgNumeric {
                        n: Some(BigDecimal::from_str(string).map_err(|_| {
                            CastError::Generic(format!("Could not parse {} into a decimal", string))
                        })?),
                    },
                };

                Box::new(decimal)
            }

            PhysicalColumnType::Timestamp { .. }
            | PhysicalColumnType::Time { .. }
            | PhysicalColumnType::Date => {
                let datetime = DateTime::parse_from_rfc3339(string);
                let naive_datetime = NaiveDateTime::parse_from_str(
                    string,
                    &format!("{}T{}", NAIVE_DATE_FORMAT, NAIVE_TIME_FORMAT),
                );

                // attempt to parse string as either datetime+offset or as a naive datetime
                match (datetime, naive_datetime) {
                    (Ok(datetime), _) => {
                        match &destination_type {
                            PhysicalColumnType::Timestamp { timezone, .. } => {
                                if *timezone {
                                    Box::new(datetime)
                                } else {
                                    // default to the naive time if this is a non-timezone field
                                    Box::new(datetime.naive_local())
                                }
                            }
                            PhysicalColumnType::Time { .. } => Box::new(datetime.time()),
                            PhysicalColumnType::Date => Box::new(datetime.date().naive_local()),
                            _ => {
                                return Err(CastError::Generic(
                                    "missing case for datetime in inner match".into(),
                                ))
                            }
                        }
                    }

                    (_, Ok(naive_datetime)) => {
                        match &destination_type {
                            PhysicalColumnType::Timestamp { timezone, .. } => {
                                if *timezone {
                                    // default to UTC+0 if this field is a timestamp+timezone field
                                    Box::new(DateTime::<Utc>::from_utc(naive_datetime, chrono::Utc))
                                } else {
                                    Box::new(naive_datetime)
                                }
                            }
                            PhysicalColumnType::Time { .. } => Box::new(naive_datetime.time()),
                            PhysicalColumnType::Date { .. } => Box::new(naive_datetime.date()),
                            _ => {
                                return Err(CastError::Generic(
                                    "missing case for datetime in inner match".into(),
                                ))
                            }
                        }
                    }

                    (Err(_), Err(_)) => {
                        match &destination_type {
                            PhysicalColumnType::Timestamp { .. } => {
                                // exhausted options for timestamp formats
                                return Err(CastError::Generic(format!(
                                    "Could not parse {} as a valid timestamp format",
                                    string
                                )));
                            }
                            PhysicalColumnType::Time { .. } => {
                                // try parsing the string as a time only
                                let t = NaiveTime::parse_from_str(string, NAIVE_TIME_FORMAT)
                                    .map_err(|e| {
                                        CastError::Date(
                                            format!(
                                                "Could not parse {} as a valid time-only format",
                                                string
                                            ),
                                            e,
                                        )
                                    })?;
                                Box::new(t)
                            }
                            PhysicalColumnType::Date => {
                                // try parsing the string as a date only
                                let d = NaiveDate::parse_from_str(string, NAIVE_DATE_FORMAT)
                                    .map_err(|e| {
                                        CastError::Date(
                                            format!(
                                                "Could not parse {} as a valid date-only format",
                                                string
                                            ),
                                            e,
                                        )
                                    })?;
                                Box::new(d)
                            }
                            _ => panic!(),
                        }
                    }
                }
            }

            PhysicalColumnType::Blob => {
                let bytes = base64::decode(string)?;
                Box::new(SQLBytes::new(bytes))
            }

            PhysicalColumnType::Uuid => {
                let uuid = uuid::Uuid::parse_str(string)?;
                Box::new(uuid)
            }

            PhysicalColumnType::Array { typ } => cast_string(string, typ)?,

            PhysicalColumnType::ColumnReference { ref_pk_type, .. } => {
                cast_string(string, ref_pk_type)?
            }

            _ => Box::new(string.to_owned()),
        };

    Ok(value)
}

fn cast_object(val: &ConstValue, destination_type: &PhysicalColumnType) -> Box<dyn SQLParam> {
    match destination_type {
        PhysicalColumnType::Json => {
            let json_object = val.clone().into_json().unwrap();
            Box::new(json_object)
        }
        _ => panic!(),
    }
}
