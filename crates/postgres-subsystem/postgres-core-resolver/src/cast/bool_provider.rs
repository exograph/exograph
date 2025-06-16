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
use exo_sql::{BooleanColumnType, PhysicalColumnType, SQLParamContainer};

pub struct BoolCastProvider;

impl CastProvider for BoolCastProvider {
    fn suitable(&self, val: &Val, destination_type: &dyn PhysicalColumnType) -> bool {
        matches!(val, Val::Bool(_)) && destination_type.as_any().is::<BooleanColumnType>()
    }

    fn cast(
        &self,
        val: &Val,
        _destination_type: &dyn PhysicalColumnType,
        _unnest: bool,
    ) -> Result<Option<SQLParamContainer>, CastError> {
        if let Val::Bool(b) = val {
            Ok(Some(SQLParamContainer::bool(*b)))
        } else {
            Err(CastError::Generic("Expected boolean value".into()))
        }
    }
}
