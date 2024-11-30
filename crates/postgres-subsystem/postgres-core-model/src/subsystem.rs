// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_plugin_interface::{error::ModelSerializationError, system_serializer::SystemSerializer};
use exo_sql::Database;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct PostgresCoreSubsystem {
    pub database: Database,
}

impl SystemSerializer for PostgresCoreSubsystem {
    type Underlying = Self;

    fn serialize(&self) -> Result<Vec<u8>, ModelSerializationError> {
        bincode::serialize(self).map_err(ModelSerializationError::Serialize)
    }

    fn deserialize_reader(
        reader: impl std::io::Read,
    ) -> Result<Self::Underlying, ModelSerializationError> {
        bincode::deserialize_from(reader).map_err(ModelSerializationError::Deserialize)
    }
}
