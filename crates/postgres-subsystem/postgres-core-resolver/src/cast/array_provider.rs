// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::CastProvider;
use crate::cast::CastError;
use common::value::Val;
use exo_sql::{
    ArrayColumnType, PhysicalColumnType, SQLParamContainer,
    array_util::{self, ArrayEntry},
    database_error::DatabaseError,
};
use tokio_postgres::types::Type;

pub struct ArrayCastProvider;

impl CastProvider for ArrayCastProvider {
    fn suitable(&self, val: &Val, _destination_type: &dyn PhysicalColumnType) -> bool {
        matches!(val, Val::List(_) | Val::String(_))
        // && destination_type.as_any().is::<ArrayColumnType>()
    }

    fn cast(
        &self,
        val: &Val,
        destination_type: &dyn PhysicalColumnType,
        unnest: bool,
    ) -> Result<Option<SQLParamContainer>, CastError> {
        if let Val::List(elems) = val {
            if unnest {
                let casted_elems: Vec<_> = elems
                    .iter()
                    .map(|e| crate::cast::cast_value(e, destination_type, false))
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(Some(
                    SQLParamContainer::new(casted_elems, array_type(destination_type)?)
                        .with_unnest(),
                ))
            } else {
                fn array_entry(elem: &Val) -> ArrayEntry<'_, Val> {
                    match elem {
                        Val::List(elems) => ArrayEntry::List(elems),
                        _ => ArrayEntry::Single(elem),
                    }
                }

                let cast_value_with_error =
                    |value: &Val| -> Result<Option<SQLParamContainer>, DatabaseError> {
                        crate::cast::cast_value(value, destination_type, false)
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
        } else if let Val::String(string) = val {
            if let Some(array_column_type) =
                destination_type.as_any().downcast_ref::<ArrayColumnType>()
            {
                let typ = &*array_column_type.typ;
                super::cast_value(val, typ, false)
            } else {
                // Fallback to string cast
                // TODO: Revisit after `Val` and schema with rich types refactoring
                Ok(Some(SQLParamContainer::string(string.clone())))
            }
        } else {
            Err(CastError::Generic("Expected list value".into()))
        }
    }
}

fn array_type(destination_type: &dyn PhysicalColumnType) -> Result<Type, CastError> {
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
