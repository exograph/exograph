// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use bytes::{Buf, Bytes};
use serde::{Serialize, de::DeserializeOwned};
use std::io::Read;

pub use crate::error::ModelSerializationError;

/// Serialize and deserialize the underlying type
/// Used to serialize and deserialize subsystems as well as the whole system
///
/// Implementations must ensure that the serialization and deserialization is
/// compatible with the same version of the underlying type. Other than that
/// there is no constraint of the serialization format. For example, one subsystem
/// may use the postcard format, while another subsystem may use JSON.
pub trait SystemSerializer {
    type Underlying;

    fn serialize(&self) -> Result<Vec<u8>, ModelSerializationError>;

    fn deserialize_reader(
        reader: impl std::io::Read,
    ) -> Result<Self::Underlying, ModelSerializationError>;

    fn deserialize(bytes: Vec<u8>) -> Result<Self::Underlying, ModelSerializationError> {
        Self::deserialize_reader(Bytes::from(bytes).reader())
    }
}

pub fn postcard_serialize<T: Serialize>(value: &T) -> Result<Vec<u8>, ModelSerializationError> {
    postcard::to_allocvec(value).map_err(ModelSerializationError::Serialize)
}

pub fn postcard_deserialize<T: DeserializeOwned>(
    mut reader: impl Read,
) -> Result<T, ModelSerializationError> {
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|e| ModelSerializationError::Other(e.to_string()))?;
    postcard::from_bytes(&bytes).map_err(ModelSerializationError::Deserialize)
}
