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
use base64::Engine;
use common::value::Val;
use exo_sql::{BlobColumnType, PhysicalColumnType, SQLParamContainer};

pub struct BlobCastProvider;

impl CastProvider for BlobCastProvider {
    fn suitable(&self, val: &Val, destination_type: &dyn PhysicalColumnType) -> bool {
        matches!(val, Val::Binary(_) | Val::String(_))
            && destination_type.as_any().is::<BlobColumnType>()
    }

    fn cast(
        &self,
        val: &Val,
        _destination_type: &dyn PhysicalColumnType,
        _unnest: bool,
    ) -> Result<Option<SQLParamContainer>, CastError> {
        if let Val::Binary(bytes) = val {
            Ok(Some(SQLParamContainer::bytes(bytes.clone())))
        } else if let Val::String(string) = val {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(string)
                .map_err(CastError::Blob)?;
            Ok(Some(SQLParamContainer::bytes_from_vec(bytes)))
        } else {
            Err(CastError::Generic("Expected binary value".into()))
        }
    }
}
