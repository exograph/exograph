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
use common::value::{Val, val::ValNumber};
use exo_sql::{FloatBits, FloatColumnType, PhysicalColumnType, SQLParamContainer};

pub struct FloatCastProvider;

impl CastProvider for FloatCastProvider {
    fn suitable(&self, val: &Val, destination_type: &dyn PhysicalColumnType) -> bool {
        matches!(val, Val::Number(_)) && destination_type.as_any().is::<FloatColumnType>()
    }

    fn cast(
        &self,
        val: &Val,
        destination_type: &dyn PhysicalColumnType,
        _unnest: bool,
    ) -> Result<Option<SQLParamContainer>, CastError> {
        if let Val::Number(number) = val {
            if let Some(float_type) = destination_type.as_any().downcast_ref::<FloatColumnType>() {
                let result = match float_type.bits {
                    FloatBits::_24 => SQLParamContainer::f32(cast_to_f32(number)?),
                    FloatBits::_53 => SQLParamContainer::f64(cast_to_f64(number)?),
                };
                Ok(Some(result))
            } else {
                Err(CastError::Generic(
                    "Expected FloatColumnType for number value".into(),
                ))
            }
        } else {
            Err(CastError::Generic("Expected number value".into()))
        }
    }
}

fn cast_to_f64(val: &ValNumber) -> Result<f64, CastError> {
    let f64_value = val
        .as_f64()
        .ok_or_else(|| CastError::Generic(format!("Failed to cast {val} to a float")))?;
    Ok(f64_value)
}

pub fn cast_to_f32(val: &ValNumber) -> Result<f32, CastError> {
    let f64_value = cast_to_f64(val)?;

    if f64_value < f32::MIN.into() || f64_value > f32::MAX.into() {
        return Err(CastError::Generic(format!(
            "Float overflow: {val} is out of range for a 32-bit float"
        )));
    }
    Ok(f64_value as f32)
}
