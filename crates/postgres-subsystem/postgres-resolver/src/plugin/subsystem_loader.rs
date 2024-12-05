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

use common::env_const::get_rest_http_path;
use postgres_core_model::subsystem::PostgresCoreSubsystem;
use postgres_graphql_resolver::PostgresSubsystemResolver;

use core_plugin_interface::{
    core_resolver::plugin::{SubsystemGraphQLResolver, SubsystemRestResolver},
    interface::{SubsystemLoader, SubsystemLoadingError, SubsystemResolver},
    serializable_system::SerializableSubsystem,
    system_serializer::SystemSerializer,
};
use exo_env::Environment;
use exo_sql::DatabaseClientManager;
use postgres_core_resolver::database_helper::create_database_executor;
use postgres_graphql_model::subsystem::PostgresGraphQLSubsystem;
use postgres_rest_model::subsystem::{PostgresRestSubsystem, PostgresRestSubsystemWithRouter};
use postgres_rest_resolver::PostgresSubsystemRestResolver;

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
        env: &dyn Environment,
    ) -> Result<Box<SubsystemResolver>, SubsystemLoadingError> {
        let executor = Arc::new(
            create_database_executor(self.existing_client.take(), env)
                .await
                .map_err(|e| SubsystemLoadingError::BoxedError(Box::new(e)))?,
        );

        let SerializableSubsystem {
            graphql,
            rest,
            core,
            ..
        } = subsystem;

        let core_subsystem = PostgresCoreSubsystem::deserialize_reader(core.0.as_slice())?;
        let core_subsystem = Arc::new(core_subsystem);

        let graphql_system = graphql
            .map(|graphql| {
                let mut subsystem = PostgresGraphQLSubsystem::deserialize(graphql.0)?;
                subsystem.core_subsystem = core_subsystem.clone();
                Ok::<_, SubsystemLoadingError>(Box::new(PostgresSubsystemResolver {
                    id: self.id(),
                    subsystem,
                    executor: executor.clone(),
                })
                    as Box<dyn SubsystemGraphQLResolver + Send + Sync>)
            })
            .transpose()?;

        let rest_system = rest
            .map(|rest| {
                let subsystem = PostgresRestSubsystem::deserialize(rest.0)?;
                let mut subsystem = PostgresRestSubsystemWithRouter::new(subsystem)?;
                subsystem.core_subsystem = core_subsystem.clone();

                let api_path_prefix = format!("{}/", get_rest_http_path(env));

                Ok::<_, SubsystemLoadingError>(Box::new(PostgresSubsystemRestResolver {
                    id: self.id(),
                    subsystem,
                    executor,
                    api_path_prefix,
                })
                    as Box<dyn SubsystemRestResolver + Send + Sync>)
            })
            .transpose()?;

        Ok(Box::new(SubsystemResolver::new(
            graphql_system,
            rest_system,
        )))
    }
}
