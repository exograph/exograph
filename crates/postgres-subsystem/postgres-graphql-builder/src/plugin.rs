// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::sync::Arc;

use core_model_builder::{error::ModelBuildingError, plugin::GraphQLSubsystemBuild};
use core_plugin_shared::{
    serializable_system::SerializableGraphQLBytes, system_serializer::SystemSerializer,
};
use postgres_core_builder::resolved_type::ResolvedTypeEnv;

pub struct PostgresGraphQLSubsystemBuilder {}

impl PostgresGraphQLSubsystemBuilder {
    pub async fn build(
        &self,
        resolved_env: &ResolvedTypeEnv<'_>,
        core_subsystem_building: Arc<postgres_core_builder::SystemContextBuilding>,
    ) -> Result<Option<GraphQLSubsystemBuild>, ModelBuildingError> {
        let subsystem = crate::system_builder::build(resolved_env, core_subsystem_building)?;
        let Some(subsystem) = subsystem else {
            return Ok(None);
        };

        let serialized_subsystem = subsystem
            .serialize()
            .map_err(ModelBuildingError::Serialize)?;

        Ok(Some(GraphQLSubsystemBuild {
            id: "postgres".to_string(),
            serialized_subsystem: SerializableGraphQLBytes(serialized_subsystem),
            query_names: {
                let pk_query_names = subsystem.pk_queries.iter().map(|(_, q)| q.name.clone());

                let collection_query_names = subsystem
                    .collection_queries
                    .iter()
                    .map(|(_, q)| q.name.clone());

                let aggregate_query_names = subsystem
                    .aggregate_queries
                    .iter()
                    .map(|(_, q)| q.name.clone());

                pk_query_names
                    .chain(collection_query_names)
                    .chain(aggregate_query_names)
                    .collect()
            },
            mutation_names: subsystem
                .mutations
                .iter()
                .map(|(_, q)| q.name.clone())
                .collect(),
            interceptions: vec![],
        }))
    }
}
