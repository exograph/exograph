// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use base64::DecodeError;
use base64::Engine;
use chrono::prelude::*;
use chrono::DateTime;
use common::value::val::ValNumber;
use common::value::Val;
use exo_sql::database_error::DatabaseError;
#[cfg(feature = "bigdecimal")]
use exo_sql::BigDecimal;
use exo_sql::ColumnPath;
use exo_sql::{
    array_util::{self, ArrayEntry},
    Column, FloatBits, IntBits, PhysicalColumn, PhysicalColumnType, SQLParamContainer,
};
#[cfg(feature = "bigdecimal")]
use std::str::FromStr;
use tokio_postgres::types::Type;

use super::postgres_execution_error::PostgresExecutionError;
use thiserror::Error;

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

pub fn literal_column(
    value: &Val,
    associated_column: &PhysicalColumn,
) -> Result<Column, PostgresExecutionError> {
    cast_value(value, &associated_column.typ, false)
        .map(|value| value.map(Column::Param).unwrap_or(Column::Null))
        .map_err(PostgresExecutionError::CastError)
}

pub fn literal_column_path(
    value: &Val,
    destination_type: &PhysicalColumnType,
    unnest: bool,
) -> Result<ColumnPath, PostgresExecutionError> {
    cast_value(value, destination_type, unnest)
        .map(|value| value.map(ColumnPath::Param).unwrap_or(ColumnPath::Null))
        .map_err(PostgresExecutionError::CastError)
}

pub fn cast_value(
    value: &Val,
    destination_type: &PhysicalColumnType,
    unnest: bool,
) -> Result<Option<SQLParamContainer>, CastError> {
    match value {
        Val::Number(number) => Ok(Some(cast_number(number, destination_type)?)),
        Val::String(v) => cast_string(v, destination_type).map(Some),
        Val::Bool(v) => Ok(Some(SQLParamContainer::bool(*v))),
        Val::Null => Ok(None),
        Val::Enum(v) => match destination_type {
            PhysicalColumnType::Enum { enum_name } => Ok(Some(SQLParamContainer::enum_(
                v.to_string(),
                enum_name.clone(),
            ))),
            _ => Err(CastError::Generic(format!(
                "Expected enum type, got {}",
                destination_type.type_string()
            ))),
        }, // We might need guidance from the database to do a correct translation
        Val::List(elems) => cast_list(elems, destination_type, unnest),
        Val::Object(_) => Ok(Some(cast_object(value, destination_type)?)),
        Val::Binary(bytes) => Ok(Some(SQLParamContainer::bytes(bytes.clone()))),
    }
}

pub fn cast_list(
    elems: &[Val],
    destination_type: &PhysicalColumnType,
    unnest: bool,
) -> Result<Option<SQLParamContainer>, CastError> {
    match destination_type {
        #[allow(unused_variables)]
        PhysicalColumnType::Vector { size } => {
            if elems.len() != *size {
                return Err(CastError::Generic(format!(
                    "Expected vector size. Expected {size}, got {}",
                    elems.len()
                )));
            }

            let vec_value: Result<Vec<f32>, CastError> = elems
                .iter()
                .map(|v| match v {
                    Val::Number(n) => cast_to_f32(n),
                    _ => Err(CastError::Generic(
                        "Invalid vector parameter: element is not of float type".into(),
                    )),
                })
                .collect();

            let vec_value = vec_value?;
            Ok(Some(SQLParamContainer::f32_array(vec_value)))
        }
        _ => {
            if unnest {
                let casted_elems: Vec<_> = elems
                    .iter()
                    .map(|e| cast_value(e, destination_type, false))
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Some(
                    SQLParamContainer::new(casted_elems, array_type(destination_type)?)
                        .with_unnest(),
                ))
            } else {
                fn array_entry(elem: &Val) -> ArrayEntry<Val> {
                    match elem {
                        Val::List(elems) => ArrayEntry::List(elems),
                        _ => ArrayEntry::Single(elem),
                    }
                }

                let cast_value_with_error =
                    |value: &Val| -> Result<Option<SQLParamContainer>, DatabaseError> {
                        cast_value(value, destination_type, false)
                            .map_err(|error| DatabaseError::BoxedError(error.into()))
                    };

                array_util::to_sql_param(
                    elems,
                    destination_type,
                    array_entry,
                    &cast_value_with_error,
                )
                .map_err(CastError::Postgres)
            }
        }
    }
}

fn cast_number(
    number: &ValNumber,
    destination_type: &PhysicalColumnType,
) -> Result<SQLParamContainer, CastError> {
    let result: SQLParamContainer = match destination_type {
        PhysicalColumnType::Int { bits } => match bits {
            IntBits::_16 => SQLParamContainer::i16(cast_to_i16(number)?),
            IntBits::_32 => SQLParamContainer::i32(cast_to_i32(number)?),
            IntBits::_64 => SQLParamContainer::i64(cast_to_i64(number)?),
        },
        PhysicalColumnType::Float { bits } => match bits {
            FloatBits::_24 => SQLParamContainer::f32(cast_to_f32(number)?),
            FloatBits::_53 => SQLParamContainer::f64(cast_to_f64(number)?),
        },
        PhysicalColumnType::Numeric { .. } => {
            return Err(CastError::Generic(
                "Number literals cannot be specified for decimal fields".into(),
            ));
        }
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
            #[cfg(feature = "bigdecimal")]
            {
                let decimal = match string {
                    "NaN" => None,
                    _ => Some(BigDecimal::from_str(string).map_err(|_| {
                        CastError::Generic(format!("Could not parse {string} into a decimal"))
                    })?),
                };

                SQLParamContainer::numeric(decimal)
            }

            #[cfg(not(feature = "bigdecimal"))]
            {
                return Err(CastError::Generic(
                    "Casting strings to decimal fields are not supported in this build".into(),
                ));
            }
        }

        #[allow(unused_variables)]
        PhysicalColumnType::Vector { size } => {
            let parsed: Vec<f32> = serde_json::from_str(string).map_err(|e| {
                CastError::Generic(format!("Could not parse {string} as a vector {e}"))
            })?;

            if parsed.len() != *size {
                return Err(CastError::Generic(format!(
                    "Expected vector of size {size}, got {}",
                    parsed.len()
                )));
            }

            SQLParamContainer::f32_array(parsed)
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
                                SQLParamContainer::timestamp_tz(datetime)
                            } else {
                                // default to the naive time if this is a non-timezone field
                                SQLParamContainer::timestamp(datetime.naive_local())
                            }
                        }
                        PhysicalColumnType::Time { .. } => SQLParamContainer::time(datetime.time()),
                        PhysicalColumnType::Date => SQLParamContainer::date(datetime.date_naive()),
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
                                SQLParamContainer::timestamp_utc(
                                    DateTime::<Utc>::from_naive_utc_and_offset(
                                        naive_datetime,
                                        chrono::Utc,
                                    ),
                                )
                            } else {
                                SQLParamContainer::timestamp(naive_datetime)
                            }
                        }
                        PhysicalColumnType::Time { .. } => {
                            SQLParamContainer::time(naive_datetime.time())
                        }
                        PhysicalColumnType::Date => SQLParamContainer::date(naive_datetime.date()),
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
                            SQLParamContainer::time(t)
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
                            SQLParamContainer::date(d)
                        }
                        _ => panic!(),
                    }
                }
            }
        }

        PhysicalColumnType::Blob => {
            let bytes = base64::engine::general_purpose::STANDARD.decode(string)?;
            SQLParamContainer::bytes_from_vec(bytes)
        }

        PhysicalColumnType::Uuid => {
            let uuid = uuid::Uuid::parse_str(string)?;
            SQLParamContainer::uuid(uuid)
        }

        PhysicalColumnType::Array { typ } => cast_string(string, typ)?,

        _ => SQLParamContainer::string(string.to_owned()),
    };

    Ok(value)
}

fn cast_object(
    val: &Val,
    destination_type: &PhysicalColumnType,
) -> Result<SQLParamContainer, CastError> {
    match destination_type {
        PhysicalColumnType::Json => {
            let json_object = val.clone().try_into().map_err(|_| {
                CastError::Generic(format!("Failed to cast {val} to a JSON object"))
            })?;
            Ok(SQLParamContainer::json(json_object))
        }
        _ => Err(CastError::Generic(format!(
            "Unexpected destination type {} for object value",
            destination_type.type_string()
        ))),
    }
}

fn array_type(destination_type: &PhysicalColumnType) -> Result<Type, CastError> {
    match destination_type.get_pg_type() {
        Type::TEXT => Ok(Type::TEXT_ARRAY),
        Type::INT4 => Ok(Type::INT4_ARRAY),
        Type::INT8 => Ok(Type::INT8_ARRAY),
        Type::FLOAT4 => Ok(Type::FLOAT4_ARRAY),
        Type::FLOAT8 => Ok(Type::FLOAT8_ARRAY),
        Type::BOOL => Ok(Type::BOOL_ARRAY),
        Type::TIMESTAMP => Ok(Type::TIMESTAMP_ARRAY),
        Type::DATE => Ok(Type::DATE_ARRAY),
        Type::TIME => Ok(Type::TIME_ARRAY),
        Type::TIMETZ => Ok(Type::TIMETZ_ARRAY),
        Type::BIT => Ok(Type::BIT_ARRAY),
        Type::VARBIT => Ok(Type::VARBIT_ARRAY),
        Type::NUMERIC => Ok(Type::NUMERIC_ARRAY),
        Type::JSONB => Ok(Type::JSONB_ARRAY),
        Type::UUID => Ok(Type::UUID_ARRAY),
        Type::JSON => Ok(Type::JSON_ARRAY),
        Type::REGPROCEDURE => Ok(Type::REGPROCEDURE_ARRAY),
        Type::REGOPER => Ok(Type::REGOPER_ARRAY),
        Type::REGOPERATOR => Ok(Type::REGOPERATOR_ARRAY),
        Type::REGCLASS => Ok(Type::REGCLASS_ARRAY),
        Type::REGTYPE => Ok(Type::REGTYPE_ARRAY),
        _ => Err(CastError::Generic("Unsupported array type".into())),
    }
}

fn cast_to_i64(val: &ValNumber) -> Result<i64, CastError> {
    let i64_value = val
        .as_i64()
        .ok_or_else(|| CastError::Generic(format!("Failed to cast {val} to an integer")))?;

    Ok(i64_value)
}

fn cast_to_i32(val: &ValNumber) -> Result<i32, CastError> {
    let i64_value = cast_to_i64(val)?;
    if i64_value < i32::MIN as i64 || i64_value > i32::MAX as i64 {
        return Err(CastError::Generic(format!(
            "Integer overflow: {val} is out of range for a 32-bit integer"
        )));
    }
    Ok(i64_value as i32)
}

fn cast_to_i16(val: &ValNumber) -> Result<i16, CastError> {
    let i32_value = cast_to_i32(val)?;
    if i32_value < i16::MIN as i32 || i32_value > i16::MAX as i32 {
        return Err(CastError::Generic(format!(
            "Integer overflow: {val} is out of range for a 16-bit integer"
        )));
    }
    Ok(i32_value as i16)
}

fn cast_to_f64(val: &ValNumber) -> Result<f64, CastError> {
    let f64_value = val
        .as_f64()
        .ok_or_else(|| CastError::Generic(format!("Failed to cast {val} to a float")))?;
    Ok(f64_value)
}

fn cast_to_f32(val: &ValNumber) -> Result<f32, CastError> {
    let f64_value = cast_to_f64(val)?;

    if f64_value < f32::MIN.into() || f64_value > f32::MAX.into() {
        return Err(CastError::Generic(format!(
            "Float overflow: {val} is out of range for a 32-bit float"
        )));
    }
    Ok(f64_value as f32)
}
