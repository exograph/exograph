// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_trait::async_trait;

use core_model_builder::{
    builder::system_builder::BaseModelSystem,
    error::ModelBuildingError,
    plugin::{
        BuildMode, CoreSubsystemBuild, GraphQLSubsystemBuild, Interception, RpcSubsystemBuild,
    },
    typechecker::{
        annotation::{AnnotationSpec, AnnotationTarget, MappedAnnotationParams},
        typ::TypecheckedSystem,
    },
};
use core_plugin_interface::interface::{SubsystemBuild, SubsystemBuilder};
use core_plugin_shared::{
    interception::InterceptorIndex,
    serializable_system::{SerializableCoreBytes, SerializableGraphQLBytes, SerializableRpcBytes},
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
                mapped_params: MappedAnnotationParams::None,
            },
        )]
    }

    async fn build(
        &self,
        typechecked_system: &TypecheckedSystem,
        base_system: &BaseModelSystem,
        build_mode: BuildMode,
    ) -> Result<Option<SubsystemBuild>, ModelBuildingError> {
        // Build the shared model (Deno-specific bundling, but protocol-agnostic)
        let module_system =
            deno_model_builder::system_builder::build(typechecked_system, base_system, build_mode)
                .await?;

        let Some(module_system) = module_system else {
            return Ok(None);
        };

        // Serialize as ModuleSubsystem for RPC (no GraphQL dependency)
        let rpc_bytes = module_system
            .underlying
            .serialize()
            .map_err(ModelBuildingError::Serialize)?;

        // Build the GraphQL wrapper from the shared model
        let graphql_subsystem = self
            .graphql_builder
            .build_from_module_system(module_system)?;

        Ok(Some(SubsystemBuild {
            id: self.id(),
            graphql: Some(graphql_subsystem),
            rest: None,
            rpc: Some(RpcSubsystemBuild {
                serialized_subsystem: SerializableRpcBytes(rpc_bytes),
            }),
            core: CoreSubsystemBuild {
                serialized_subsystem: SerializableCoreBytes(vec![]),
            },
        }))
    }
}

struct DenoGraphQLSubsystemBuilder {}

impl DenoGraphQLSubsystemBuilder {
    fn build_from_module_system(
        &self,
        module_system: subsystem_model_builder_util::ModuleSubsystemWithInterceptors,
    ) -> Result<GraphQLSubsystemBuild, ModelBuildingError> {
        let ModelDenoSystemWithInterceptors {
            underlying: subsystem,
            interceptors,
        } = deno_graphql_builder::system_builder::build_from_module_system(module_system);

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

        Ok(GraphQLSubsystemBuild {
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
        })
    }
}
