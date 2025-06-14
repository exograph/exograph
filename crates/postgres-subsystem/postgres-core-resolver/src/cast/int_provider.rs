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
use exo_sql::{IntBits, IntColumnType, PhysicalColumnType, SQLParamContainer};

pub struct IntCastProvider;

impl CastProvider for IntCastProvider {
    fn suitable(&self, val: &Val, destination_type: &dyn PhysicalColumnType) -> bool {
        matches!(val, Val::Number(_)) && destination_type.as_any().is::<IntColumnType>()
    }

    fn cast(
        &self,
        val: &Val,
        destination_type: &dyn PhysicalColumnType,
        _unnest: bool,
    ) -> Result<Option<SQLParamContainer>, CastError> {
        if let Val::Number(number) = val {
            if let Some(int_type) = destination_type.as_any().downcast_ref::<IntColumnType>() {
                let result = match int_type.bits {
                    IntBits::_16 => SQLParamContainer::i16(cast_to_i16(number)?),
                    IntBits::_32 => SQLParamContainer::i32(cast_to_i32(number)?),
                    IntBits::_64 => SQLParamContainer::i64(cast_to_i64(number)?),
                };
                Ok(Some(result))
            } else {
                Err(CastError::Generic(
                    "Expected IntColumnType for number value".into(),
                ))
            }
        } else {
            Err(CastError::Generic("Expected number value".into()))
        }
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
    let i64_value = cast_to_i64(val)?;
    if i64_value < i16::MIN as i64 || i64_value > i16::MAX as i64 {
        return Err(CastError::Generic(format!(
            "Integer overflow: {val} is out of range for a 16-bit integer"
        )));
    }
    Ok(i64_value as i16)
}
