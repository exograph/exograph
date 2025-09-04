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

use common::env_const::{get_rest_http_path, get_rpc_http_path};
use postgres_core_model::subsystem::PostgresCoreSubsystem;
use postgres_graphql_resolver::PostgresSubsystemResolver;

use core_plugin_interface::interface::{SubsystemLoader, SubsystemLoadingError, SubsystemResolver};
use core_plugin_shared::{
    serializable_system::SerializableSubsystem, system_serializer::SystemSerializer,
};
use core_resolver::plugin::{
    SubsystemGraphQLResolver, SubsystemRestResolver, SubsystemRpcResolver,
};
use exo_env::Environment;
use exo_sql::DatabaseClientManager;
use postgres_core_resolver::database_helper::create_database_executor;
use postgres_graphql_model::subsystem::PostgresGraphQLSubsystem;
use postgres_rest_model::subsystem::{PostgresRestSubsystem, PostgresRestSubsystemWithRouter};
use postgres_rest_resolver::PostgresSubsystemRestResolver;
use postgres_rpc_model::subsystem::{PostgresRpcSubsystem, PostgresRpcSubsystemWithRouter};
use postgres_rpc_resolver::PostgresSubsystemRpcResolver;

pub struct PostgresSubsystemLoader {
    pub existing_client: Option<DatabaseClientManager>,
}

#[async_trait]
impl SubsystemLoader for PostgresSubsystemLoader {
    fn id(&self) -> &'static str {
        "postgres"
    }

    async fn init(
        &mut self,
        subsystem: SerializableSubsystem,
        env: Arc<dyn Environment>,
    ) -> Result<Box<SubsystemResolver>, SubsystemLoadingError> {
        let executor = Arc::new(
            create_database_executor(self.existing_client.take(), env.as_ref())
                .await
                .map_err(|e| SubsystemLoadingError::BoxedError(Box::new(e)))?,
        );

        let SerializableSubsystem {
            graphql,
            rest,
            rpc,
            core,
            ..
        } = subsystem;

        let core_subsystem = PostgresCoreSubsystem::deserialize_reader(core.0.as_slice())?;
        let core_subsystem = Arc::new(core_subsystem);

        let graphql_system = graphql
            .map(|graphql| {
                let mut subsystem = PostgresGraphQLSubsystem::deserialize(graphql.0)?;
                subsystem.core_subsystem = core_subsystem.clone();
                Ok::<_, SubsystemLoadingError>(Arc::new(PostgresSubsystemResolver {
                    id: self.id(),
                    subsystem,
                    executor: executor.clone(),
                })
                    as Arc<dyn SubsystemGraphQLResolver + Send + Sync>)
            })
            .transpose()?;

        let rest_system = rest
            .map(|rest| {
                let subsystem = PostgresRestSubsystem::deserialize(rest.0)?;
                let mut subsystem = PostgresRestSubsystemWithRouter::new(subsystem)?;
                subsystem.core_subsystem = core_subsystem.clone();

                let api_path_prefix = format!("{}/", get_rest_http_path(env.as_ref()));

                Ok::<_, SubsystemLoadingError>(Box::new(PostgresSubsystemRestResolver {
                    id: self.id(),
                    subsystem,
                    executor: executor.clone(),
                    api_path_prefix,
                })
                    as Box<dyn SubsystemRestResolver + Send + Sync>)
            })
            .transpose()?;

        let rpc_system = rpc
            .map(|rpc| {
                let subsystem = PostgresRpcSubsystem::deserialize(rpc.0)?;
                let mut subsystem = PostgresRpcSubsystemWithRouter::new(subsystem)?;
                subsystem.core_subsystem = core_subsystem.clone();
                let api_path_prefix = get_rpc_http_path(env.as_ref()).to_string();
                Ok::<_, SubsystemLoadingError>(Box::new(PostgresSubsystemRpcResolver {
                    id: self.id(),
                    subsystem,
                    executor: executor.clone(),
                    api_path_prefix,
                })
                    as Box<dyn SubsystemRpcResolver + Send + Sync>)
            })
            .transpose()?;

        Ok(Box::new(SubsystemResolver::new(
            graphql_system,
            rest_system,
            rpc_system,
        )))
    }
}
