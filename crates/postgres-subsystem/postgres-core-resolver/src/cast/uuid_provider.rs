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
use exo_sql::{PhysicalColumnType, SQLParamContainer, UuidColumnType};

pub struct UuidCastProvider;

impl CastProvider for UuidCastProvider {
    fn suitable(&self, val: &Val, destination_type: &dyn PhysicalColumnType) -> bool {
        matches!(val, Val::String(_)) && destination_type.as_any().is::<UuidColumnType>()
    }

    fn cast(
        &self,
        val: &Val,
        _destination_type: &dyn PhysicalColumnType,
        _unnest: bool,
    ) -> Result<Option<SQLParamContainer>, CastError> {
        match val {
            Val::String(string) => {
                let uuid = uuid::Uuid::parse_str(string).map_err(CastError::Uuid)?;
                Ok(Some(SQLParamContainer::uuid(uuid)))
            }
            _ => Err(CastError::Generic("Unexpected value type for uuid".into())),
        }
    }
}
