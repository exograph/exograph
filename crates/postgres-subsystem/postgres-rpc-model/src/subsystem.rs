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
use core_plugin_shared::system_serializer::{
    ModelSerializationError, SystemSerializer, postcard_deserialize, postcard_serialize,
};
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
        postcard_serialize(self)
    }

    fn deserialize_reader(
        reader: impl std::io::Read,
    ) -> Result<Self::Underlying, ModelSerializationError> {
        postcard_deserialize(reader)
    }
}
