// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_trait::async_trait;

use core_plugin_interface::{
    core_model_builder::{
        builder::system_builder::BaseModelSystem,
        error::ModelBuildingError,
        plugin::{CoreSubsystemBuild, GraphQLSubsystemBuild, Interception},
        typechecker::{
            annotation::{AnnotationSpec, AnnotationTarget},
            typ::TypecheckedSystem,
        },
    },
    interception::InterceptorIndex,
    interface::{GraphQLSubsystemBuilder, SubsystemBuild, SubsystemBuilder},
    serializable_system::{SerializableCoreBytes, SerializableGraphQLBytes},
    system_serializer::SystemSerializer,
};

use deno_graphql_builder::system_builder::ModelDenoSystemWithInterceptors;

pub struct DenoSubsystemBuilder {
    graphql_builder: DenoGraphQLSubsystemBuilder,
}

impl Default for DenoSubsystemBuilder {
    fn default() -> Self {
        Self {
            graphql_builder: DenoGraphQLSubsystemBuilder {},
        }
    }
}

#[async_trait]
impl SubsystemBuilder for DenoSubsystemBuilder {
    fn id(&self) -> &'static str {
        "deno"
    }

    fn annotations(&self) -> Vec<(&'static str, AnnotationSpec)> {
        vec![(
            "deno",
            AnnotationSpec {
                targets: &[AnnotationTarget::Module],
                no_params: false,
                single_params: true,
                mapped_params: None,
            },
        )]
    }

    async fn build(
        &self,
        typechecked_system: &TypecheckedSystem,
        base_system: &BaseModelSystem,
    ) -> Result<Option<SubsystemBuild>, ModelBuildingError> {
        let graphql_subsystem = self
            .graphql_builder
            .build(typechecked_system, base_system)
            .await?;

        Ok(graphql_subsystem.map(|graphql_subsystem| SubsystemBuild {
            id: self.id(),
            graphql: Some(graphql_subsystem),
            rest: None,
            rpc: None,
            core: CoreSubsystemBuild {
                serialized_subsystem: SerializableCoreBytes(vec![]),
            },
        }))
    }
}

struct DenoGraphQLSubsystemBuilder {}

#[async_trait]
impl GraphQLSubsystemBuilder for DenoGraphQLSubsystemBuilder {
    fn id(&self) -> &'static str {
        "deno"
    }

    async fn build(
        &self,
        typechecked_system: &TypecheckedSystem,
        base_system: &BaseModelSystem,
    ) -> Result<Option<GraphQLSubsystemBuild>, ModelBuildingError> {
        let subsystem =
            deno_graphql_builder::system_builder::build(typechecked_system, base_system).await?;

        let Some(ModelDenoSystemWithInterceptors {
            underlying: subsystem,
            interceptors,
        }) = subsystem
        else {
            return Ok(None);
        };

        let serialized_subsystem = subsystem
            .serialize()
            .map_err(ModelBuildingError::Serialize)?;

        let interceptions = interceptors
            .into_iter()
            .map(|(expr, index)| {
                let interceptor = &subsystem.interceptors[index];
                let kind = interceptor.interceptor_kind.clone();

                Interception {
                    expr,
                    kind,
                    index: InterceptorIndex(index.to_idx()),
                }
            })
            .collect();

        Ok(Some(GraphQLSubsystemBuild {
            id: "deno".to_string(),
            serialized_subsystem: SerializableGraphQLBytes(serialized_subsystem),
            query_names: subsystem
                .queries
                .iter()
                .map(|(_, q)| q.name.clone())
                .collect(),
            mutation_names: subsystem
                .mutations
                .iter()
                .map(|(_, q)| q.name.clone())
                .collect(),
            interceptions,
        }))
    }
}
