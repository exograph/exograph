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

use deno_graphql_model::subsystem::DenoSubsystem;
use deno_graphql_resolver::{exo_config, DenoSubsystemResolver};
use exo_deno::DenoExecutorPool;
use exo_env::Environment;

pub struct DenoSubsystemLoader {}

#[async_trait]
impl SubsystemLoader for DenoSubsystemLoader {
    fn id(&self) -> &'static str {
        "deno"
    }

    async fn init(
        &mut self,
        serialized_subsystem: SerializableSubsystem,
        _env: &dyn Environment,
    ) -> Result<Box<SubsystemResolver>, SubsystemLoadingError> {
        deno_core::JsRuntime::init_platform(None, true);

        let graphql = match serialized_subsystem.graphql {
            Some(graphql) => {
                let subsystem = DenoSubsystem::deserialize(graphql.0)?;
                let executor = DenoExecutorPool::new_from_config(exo_config());
                Ok::<_, SubsystemLoadingError>(Some(Box::new(DenoSubsystemResolver {
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
