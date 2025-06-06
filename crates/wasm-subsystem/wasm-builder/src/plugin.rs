// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::vec;

use async_trait::async_trait;

use core_model_builder::{
    builder::system_builder::BaseModelSystem,
    error::ModelBuildingError,
    plugin::{BuildMode, CoreSubsystemBuild},
    typechecker::{
        annotation::{AnnotationSpec, AnnotationTarget},
        typ::TypecheckedSystem,
    },
};

use core_plugin_interface::interface::{GraphQLSubsystemBuilder, SubsystemBuild, SubsystemBuilder};
use core_plugin_shared::serializable_system::SerializableCoreBytes;

use wasm_graphql_builder::GraphQLWasmSubsystemBuilder;

pub struct WasmSubsystemBuilder {
    pub graphql_builder: GraphQLWasmSubsystemBuilder,
}

impl Default for WasmSubsystemBuilder {
    fn default() -> Self {
        Self {
            graphql_builder: GraphQLWasmSubsystemBuilder {},
        }
    }
}

#[async_trait]
impl SubsystemBuilder for WasmSubsystemBuilder {
    fn id(&self) -> &'static str {
        "wasm"
    }

    fn annotations(&self) -> Vec<(&'static str, AnnotationSpec)> {
        vec![(
            "wasm",
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
        build_mode: BuildMode,
    ) -> Result<Option<SubsystemBuild>, ModelBuildingError> {
        let graphql_subsystem = self
            .graphql_builder
            .build(typechecked_system, base_system, build_mode)
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
