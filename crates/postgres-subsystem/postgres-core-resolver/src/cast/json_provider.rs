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
use exo_sql::{JsonColumnType, PhysicalColumnType, SQLParamContainer};

pub struct JsonCastProvider;

impl CastProvider for JsonCastProvider {
    fn suitable(&self, _val: &Val, destination_type: &dyn PhysicalColumnType) -> bool {
        // Any value can be cast to a JSON column
        destination_type.as_any().is::<JsonColumnType>()
    }

    fn cast(
        &self,
        val: &Val,
        destination_type: &dyn PhysicalColumnType,
        _unnest: bool,
    ) -> Result<Option<SQLParamContainer>, CastError> {
        if destination_type.as_any().is::<JsonColumnType>() {
            let json_object = val.clone().try_into().map_err(|_| {
                CastError::Generic(format!("Failed to cast {val} to a JSON object"))
            })?;
            Ok(Some(SQLParamContainer::json(json_object)))
        } else {
            Err(CastError::Generic(format!(
                "Unexpected destination type {} for object value",
                destination_type.type_string()
            )))
        }
    }
}
