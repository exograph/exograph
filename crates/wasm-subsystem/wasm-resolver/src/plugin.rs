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
    core_resolver::plugin::SubsystemGraphQLResolver,
    interface::{SubsystemLoader, SubsystemLoadingError, SubsystemResolver},
    serializable_system::SerializableSubsystem,
    system_serializer::SystemSerializer,
};
use exo_env::Environment;
use exo_wasm::WasmExecutorPool;
use wasm_graphql_model::subsystem::WasmSubsystem;
use wasm_graphql_resolver::WasmSubsystemResolver;

pub struct WasmSubsystemLoader {}

#[async_trait]
impl SubsystemLoader for WasmSubsystemLoader {
    fn id(&self) -> &'static str {
        "wasm"
    }

    async fn init(
        &mut self,
        serialized_subsystem: SerializableSubsystem,
        _env: &dyn Environment,
    ) -> Result<Box<SubsystemResolver>, SubsystemLoadingError> {
        let executor = WasmExecutorPool::default();

        let graphql = match serialized_subsystem.graphql {
            Some(graphql) => {
                let subsystem = WasmSubsystem::deserialize(graphql.0)?;

                Ok::<_, SubsystemLoadingError>(Some(Box::new(WasmSubsystemResolver {
                    id: self.id(),
                    subsystem,
                    executor,
                })
                    as Box<dyn SubsystemGraphQLResolver + Send + Sync>))
            }
            None => Ok(None),
        }?;

        Ok(Box::new(SubsystemResolver::new(graphql, None)))
    }
}
