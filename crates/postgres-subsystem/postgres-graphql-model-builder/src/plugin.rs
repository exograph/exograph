// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_plugin_interface::{
    async_trait::async_trait,
    core_model_builder::{
        builder::system_builder::BaseModelSystem, error::ModelBuildingError,
        plugin::GraphQLSubsystemBuild, typechecker::typ::TypecheckedSystem,
    },
    interface::GraphQLSubsystemBuilder,
    serializable_system::SerializableGraphQLBytes,
    system_serializer::SystemSerializer,
};

pub struct PostgresGraphQLSubsystemBuilder {}

#[async_trait]
impl GraphQLSubsystemBuilder for PostgresGraphQLSubsystemBuilder {
    fn id(&self) -> &'static str {
        "postgres/graphql"
    }

    async fn build(
        &self,
        typechecked_system: &TypecheckedSystem,
        base_system: &BaseModelSystem,
    ) -> Result<Option<GraphQLSubsystemBuild>, ModelBuildingError> {
        let subsystem = crate::system_builder::build(typechecked_system, base_system)?;
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
