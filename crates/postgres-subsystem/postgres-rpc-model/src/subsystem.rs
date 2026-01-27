// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::sync::Arc;

use core_model::mapped_arena::MappedArena;
use core_plugin_interface::interface::SubsystemLoadingError;
use core_plugin_shared::{error::ModelSerializationError, system_serializer::SystemSerializer};
use postgres_core_model::subsystem::PostgresCoreSubsystem;
use serde::{Deserialize, Serialize};

use crate::operation::{CollectionQuery, PkQuery};

#[derive(Serialize, Deserialize, Debug)]
pub struct PostgresRpcSubsystem {
    pub pk_queries: MappedArena<PkQuery>,
    pub collection_queries: MappedArena<CollectionQuery>,
    // Future: pub mutations: MappedArena<RpcMutation>,
    #[serde(skip)]
    pub core_subsystem: Arc<PostgresCoreSubsystem>,
}

#[derive(Debug)]
pub struct PostgresRpcSubsystemWithRouter {
    pub pk_queries: MappedArena<PkQuery>,
    pub collection_queries: MappedArena<CollectionQuery>,
    pub core_subsystem: Arc<PostgresCoreSubsystem>,
}

impl PostgresRpcSubsystemWithRouter {
    pub fn new(subsystem: PostgresRpcSubsystem) -> Result<Self, SubsystemLoadingError> {
        Ok(Self {
            pk_queries: subsystem.pk_queries,
            collection_queries: subsystem.collection_queries,
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
