// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::{CastProvider, float_provider::cast_to_f32};
use crate::cast::CastError;
use common::value::Val;
use exo_sql::{PhysicalColumnType, SQLParamContainer, VectorColumnType};

pub struct VectorCastProvider;

impl CastProvider for VectorCastProvider {
    fn suitable(&self, val: &Val, destination_type: &dyn PhysicalColumnType) -> bool {
        matches!(val, Val::List(_) | Val::String(_))
            && destination_type.as_any().is::<VectorColumnType>()
    }

    fn cast(
        &self,
        val: &Val,
        destination_type: &dyn PhysicalColumnType,
        _unnest: bool,
    ) -> Result<Option<SQLParamContainer>, CastError> {
        let vector_column_type = destination_type
            .as_any()
            .downcast_ref::<VectorColumnType>()
            .unwrap();

        let size = &vector_column_type.size;

        if let Val::List(elems) = val {
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
        } else if let Val::String(string) = val {
            let parsed: Vec<f32> = serde_json::from_str(string).map_err(|e| {
                CastError::Generic(format!("Could not parse {string} as a vector {e}"))
            })?;

            if parsed.len() != *size {
                return Err(CastError::Generic(format!(
                    "Expected vector of size {size}, got {}",
                    parsed.len()
                )));
            }

            Ok(Some(SQLParamContainer::f32_array(parsed)))
        } else {
            Err(CastError::Generic("Expected list value".into()))
        }
    }
}
