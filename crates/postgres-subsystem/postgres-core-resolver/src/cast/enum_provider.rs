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
use exo_sql::{EnumColumnType, PhysicalColumnType, SQLParamContainer};

pub struct EnumCastProvider;

impl CastProvider for EnumCastProvider {
    fn suitable(&self, val: &Val, destination_type: &dyn PhysicalColumnType) -> bool {
        matches!(val, Val::Enum(_)) && destination_type.as_any().is::<EnumColumnType>()
    }

    fn cast(
        &self,
        val: &Val,
        destination_type: &dyn PhysicalColumnType,
        _unnest: bool,
    ) -> Result<Option<SQLParamContainer>, CastError> {
        if let Val::Enum(enum_val) = val {
            if let Some(enum_type) = destination_type.as_any().downcast_ref::<EnumColumnType>() {
                let enum_name = &enum_type.enum_name;
                Ok(Some(SQLParamContainer::enum_(
                    enum_val.to_string(),
                    enum_name.clone(),
                )))
            } else {
                Err(CastError::Generic(format!(
                    "Expected enum type, got {}",
                    destination_type.type_string()
                )))
            }
        } else {
            Err(CastError::Generic("Expected enum value".into()))
        }
    }
}
