// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::sync::Arc;

use async_trait::async_trait;

use core_plugin_interface::interface::{SubsystemLoader, SubsystemLoadingError, SubsystemResolver};

use core_plugin_shared::{
    serializable_system::SerializableSubsystem, system_serializer::SystemSerializer,
};
use core_resolver::plugin::{SubsystemGraphQLResolver, SubsystemRpcResolver};

use deno_core_resolver::{ExoDenoExecutorPool, exo_config};
use deno_graphql_model::subsystem::DenoSubsystem;
use deno_graphql_resolver::DenoSubsystemGraphQLResolver;
use deno_rpc_resolver::{DenoRpcExecutor, DenoSubsystemRpcResolver};
use exo_deno::DenoExecutorPool;
use exo_env::Environment;
use subsystem_model_util::subsystem::ModuleSubsystem;

pub struct DenoSubsystemLoader {}

#[async_trait]
impl SubsystemLoader for DenoSubsystemLoader {
    fn id(&self) -> &'static str {
        "deno"
    }

    async fn init(
        &mut self,
        serialized_subsystem: SerializableSubsystem,
        env: Arc<dyn Environment>,
    ) -> Result<Box<SubsystemResolver>, SubsystemLoadingError> {
        exo_deno::initialize();

        // Single shared executor pool, created on first use
        let mut shared_executor: Option<Arc<ExoDenoExecutorPool>> = None;
        let mut get_executor = || {
            shared_executor
                .get_or_insert_with(|| {
                    Arc::new(DenoExecutorPool::new_from_config(exo_config(env.clone())))
                })
                .clone()
        };

        let graphql = serialized_subsystem
            .graphql
            .map(|graphql| {
                let subsystem = DenoSubsystem::deserialize(graphql.0)?;
                Ok::<_, SubsystemLoadingError>(Arc::new(DenoSubsystemGraphQLResolver {
                    id: self.id(),
                    subsystem,
                    executor: get_executor(),
                })
                    as Arc<dyn SubsystemGraphQLResolver + Send + Sync>)
            })
            .transpose()?;

        let rpc = serialized_subsystem
            .rpc
            .map(|rpc| {
                let subsystem = ModuleSubsystem::deserialize(rpc.0)?;
                let deno_executor = DenoRpcExecutor {
                    executor: get_executor(),
                };
                Ok::<_, SubsystemLoadingError>(Box::new(DenoSubsystemRpcResolver::new(
                    self.id(),
                    subsystem,
                    deno_executor,
                ))
                    as Box<dyn SubsystemRpcResolver + Send + Sync>)
            })
            .transpose()?;

        Ok(Box::new(SubsystemResolver::new(graphql, None, rpc)))
    }
}
