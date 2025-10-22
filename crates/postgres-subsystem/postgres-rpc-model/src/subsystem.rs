// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;
use std::sync::Arc;

use core_plugin_interface::interface::SubsystemLoadingError;
use core_plugin_shared::{error::ModelSerializationError, system_serializer::SystemSerializer};
use postgres_core_model::subsystem::PostgresCoreSubsystem;
use serde::{Deserialize, Serialize};

use crate::operation::PostgresOperation;

#[derive(Serialize, Deserialize, Debug)]
pub struct PostgresRpcSubsystem {
    pub operations: Vec<(String, PostgresOperation)>,
    #[serde(skip)]
    pub core_subsystem: Arc<PostgresCoreSubsystem>,
}

#[derive(Debug)]
pub struct PostgresRpcSubsystemWithRouter {
    pub method_operation_map: HashMap<String, PostgresOperation>,
    pub core_subsystem: Arc<PostgresCoreSubsystem>,
}

impl PostgresRpcSubsystemWithRouter {
    pub fn new(subsystem: PostgresRpcSubsystem) -> Result<Self, SubsystemLoadingError> {
        let method_operation_map = HashMap::from_iter(subsystem.operations);
        Ok(Self {
            method_operation_map,
            core_subsystem: subsystem.core_subsystem.clone(),
        })
    }
}

impl SystemSerializer for PostgresRpcSubsystem {
    type Underlying = Self;

    fn serialize(&self) -> Result<Vec<u8>, ModelSerializationError> {
        bincode::serde::encode_to_vec(self, bincode::config::standard())
            .map_err(ModelSerializationError::Serialize)
    }

    fn deserialize_reader(
        mut reader: impl std::io::Read,
    ) -> Result<Self::Underlying, ModelSerializationError> {
        bincode::serde::decode_from_std_read(&mut reader, bincode::config::standard())
            .map_err(ModelSerializationError::Deserialize)
    }
}
