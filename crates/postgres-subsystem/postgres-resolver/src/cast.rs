// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use base64::DecodeError;
use chrono::prelude::*;
use chrono::DateTime;
use core_plugin_interface::core_resolver::value::Val;
use exo_sql::database_error::DatabaseError;
use exo_sql::{
    array_util::{self, ArrayEntry},
    Column, FloatBits, IntBits, PhysicalColumn, PhysicalColumnType, SQLBytes, SQLParamContainer,
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
    Postgres(#[from] exo_sql::database_error::DatabaseError),
}

pub(crate) fn literal_column(
    value: &Val,
    associated_column: &PhysicalColumn,
) -> Result<Column, PostgresExecutionError> {
    cast_value(value, &associated_column.typ)
        .map(|value| value.map(Column::Param).unwrap_or(Column::Null))
        .map_err(PostgresExecutionError::CastError)
}

pub(crate) fn cast_value(
    value: &Val,
    destination_type: &PhysicalColumnType,
) -> Result<Option<SQLParamContainer>, CastError> {
    match value {
        Val::Number(number) => cast_number(number, destination_type).map(Some),
        Val::String(v) => cast_string(v, destination_type).map(Some),
        Val::Bool(v) => Ok(Some(SQLParamContainer::new(*v))),
        Val::Null => Ok(None),
        Val::Enum(v) => Ok(Some(SQLParamContainer::new(v.to_string()))), // We might need guidance from the database to do a correct translation
        Val::List(elems) => cast_list(elems, destination_type),
        Val::Object(_) => Ok(Some(cast_object(value, destination_type))),
        Val::Binary(bytes) => Ok(Some(SQLParamContainer::new(SQLBytes(bytes.clone())))),
    }
}

fn cast_list(
    elems: &[Val],
    destination_type: &PhysicalColumnType,
) -> Result<Option<SQLParamContainer>, CastError> {
    fn array_entry(elem: &Val) -> ArrayEntry<Val> {
        match elem {
            Val::List(elems) => ArrayEntry::List(elems),
            _ => ArrayEntry::Single(elem),
        }
    }

    let cast_value_with_error = |value: &Val| -> Result<Option<SQLParamContainer>, DatabaseError> {
        cast_value(value, destination_type).map_err(|error| DatabaseError::BoxedError(error.into()))
    };

    array_util::to_sql_param(elems, array_entry, &cast_value_with_error)
        .map_err(CastError::Postgres)
}

fn cast_number(
    number: &serde_json::Number,
    destination_type: &PhysicalColumnType,
) -> Result<SQLParamContainer, CastError> {
    let result: SQLParamContainer = match destination_type {
        PhysicalColumnType::Int { bits } => match bits {
            IntBits::_16 => SQLParamContainer::new(number.as_i64().unwrap() as i16),
            IntBits::_32 => SQLParamContainer::new(number.as_i64().unwrap() as i32),
            IntBits::_64 => SQLParamContainer::new(number.as_i64().unwrap()),
        },
        PhysicalColumnType::Float { bits } => match bits {
            FloatBits::_24 => SQLParamContainer::new(number.as_f64().unwrap() as f32),
            FloatBits::_53 => SQLParamContainer::new(number.as_f64().unwrap()),
        },
        PhysicalColumnType::Numeric { .. } => {
            return Err(CastError::Generic(
                "Number literals cannot be specified for decimal fields".into(),
            ));
        }
        PhysicalColumnType::OneToMany(_, ref_pk_type) => {
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
) -> Result<SQLParamContainer, CastError> {
    let value: SQLParamContainer = match destination_type {
        PhysicalColumnType::Numeric { .. } => {
            let decimal = match string {
                "NaN" => PgNumeric { n: None },
                _ => PgNumeric {
                    n: Some(BigDecimal::from_str(string).map_err(|_| {
                        CastError::Generic(format!("Could not parse {string} into a decimal"))
                    })?),
                },
            };

            SQLParamContainer::new(decimal)
        }

        PhysicalColumnType::Timestamp { .. }
        | PhysicalColumnType::Time { .. }
        | PhysicalColumnType::Date => {
            let datetime = DateTime::parse_from_rfc3339(string);
            let naive_datetime = NaiveDateTime::parse_from_str(
                string,
                &format!("{NAIVE_DATE_FORMAT}T{NAIVE_TIME_FORMAT}"),
            );

            // attempt to parse string as either datetime+offset or as a naive datetime
            match (datetime, naive_datetime) {
                (Ok(datetime), _) => {
                    match &destination_type {
                        PhysicalColumnType::Timestamp { timezone, .. } => {
                            if *timezone {
                                SQLParamContainer::new(datetime)
                            } else {
                                // default to the naive time if this is a non-timezone field
                                SQLParamContainer::new(datetime.naive_local())
                            }
                        }
                        PhysicalColumnType::Time { .. } => SQLParamContainer::new(datetime.time()),
                        PhysicalColumnType::Date => SQLParamContainer::new(datetime.date_naive()),
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
                                SQLParamContainer::new(DateTime::<Utc>::from_utc(
                                    naive_datetime,
                                    chrono::Utc,
                                ))
                            } else {
                                SQLParamContainer::new(naive_datetime)
                            }
                        }
                        PhysicalColumnType::Time { .. } => {
                            SQLParamContainer::new(naive_datetime.time())
                        }
                        PhysicalColumnType::Date { .. } => {
                            SQLParamContainer::new(naive_datetime.date())
                        }
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
                                "Could not parse {string} as a valid timestamp format"
                            )));
                        }
                        PhysicalColumnType::Time { .. } => {
                            // try parsing the string as a time only
                            let t = NaiveTime::parse_from_str(string, NAIVE_TIME_FORMAT).map_err(
                                |e| {
                                    CastError::Date(
                                        format!(
                                            "Could not parse {string} as a valid time-only format"
                                        ),
                                        e,
                                    )
                                },
                            )?;
                            SQLParamContainer::new(t)
                        }
                        PhysicalColumnType::Date => {
                            // try parsing the string as a date only
                            let d = NaiveDate::parse_from_str(string, NAIVE_DATE_FORMAT).map_err(
                                |e| {
                                    CastError::Date(
                                        format!(
                                            "Could not parse {string} as a valid date-only format"
                                        ),
                                        e,
                                    )
                                },
                            )?;
                            SQLParamContainer::new(d)
                        }
                        _ => panic!(),
                    }
                }
            }
        }

        PhysicalColumnType::Blob => {
            let bytes = base64::decode(string)?;
            SQLParamContainer::new(SQLBytes::new(bytes))
        }

        PhysicalColumnType::Uuid => {
            let uuid = uuid::Uuid::parse_str(string)?;
            SQLParamContainer::new(uuid)
        }

        PhysicalColumnType::Array { typ } => cast_string(string, typ)?,

        PhysicalColumnType::OneToMany(_, ref_pk_type) => cast_string(string, ref_pk_type)?,

        _ => SQLParamContainer::new(string.to_owned()),
    };

    Ok(value)
}

fn cast_object(val: &Val, destination_type: &PhysicalColumnType) -> SQLParamContainer {
    match destination_type {
        PhysicalColumnType::Json => {
            let json_object = val.clone().into_json().unwrap();
            SQLParamContainer::new(json_object)
        }
        _ => panic!(),
    }
}
