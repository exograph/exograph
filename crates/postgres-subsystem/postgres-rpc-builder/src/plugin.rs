// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::sync::Arc;

use core_model_builder::{error::ModelBuildingError, plugin::RpcSubsystemBuild};
use core_plugin_shared::{
    serializable_system::SerializableRpcBytes, system_serializer::SystemSerializer,
};
use postgres_core_builder::resolved_type::ResolvedTypeEnv;

pub struct PostgresRpcSubsystemBuilder {}

impl PostgresRpcSubsystemBuilder {
    pub async fn build(
        &self,
        resolved_env: &ResolvedTypeEnv<'_>,
        core_subsystem_building: Arc<postgres_core_builder::SystemContextBuilding>,
    ) -> Result<Option<RpcSubsystemBuild>, ModelBuildingError> {
        let subsystem = crate::system_builder::build(resolved_env, core_subsystem_building)?;
        let Some(subsystem) = subsystem else {
            return Ok(None);
        };

        let serialized_subsystem = subsystem
            .serialize()
            .map_err(ModelBuildingError::Serialize)?;

        Ok(Some(RpcSubsystemBuild {
            serialized_subsystem: SerializableRpcBytes(serialized_subsystem),
        }))
    }
}
