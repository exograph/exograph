// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::PostgresSubsystemResolver;
use async_trait::async_trait;
#[cfg(feature = "network")]
use common::env_const::{DATABASE_URL, EXO_POSTGRES_URL};
use core_plugin_interface::{
    core_resolver::plugin::SubsystemResolver,
    interface::{SubsystemLoader, SubsystemLoadingError},
    system_serializer::SystemSerializer,
};
use exo_env::Environment;
use exo_sql::{DatabaseClientManager, DatabaseExecutor};
use postgres_model::subsystem::PostgresSubsystem;

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
        serialized_subsystem: Vec<u8>,
        env: &dyn Environment,
    ) -> Result<Box<dyn SubsystemResolver + Send + Sync>, SubsystemLoadingError> {
        let subsystem = PostgresSubsystem::deserialize(serialized_subsystem)?;

        let database_client = if let Some(existing) = self.existing_client.take() {
            existing
        } else {
            #[cfg(feature = "network")]
            {
                let url = env
                    .get(EXO_POSTGRES_URL)
                    .or(env.get(DATABASE_URL))
                    .ok_or_else(|| {
                        SubsystemLoadingError::Config("Env EXO_POSTGRES_URL not set".to_string())
                    })?;
                let pool_size: Option<usize> = env
                    .get("EXO_CONNECTION_POOL_SIZE")
                    .and_then(|s| s.parse().ok());
                let check_connection = env
                    .get("EXO_CHECK_CONNECTION_ON_STARTUP")
                    .map(|s| s == "true")
                    .unwrap_or(true);

                DatabaseClientManager::from_url(&url, check_connection, pool_size)
                    .await
                    .map_err(|e| SubsystemLoadingError::BoxedError(Box::new(e)))?
            }

            #[cfg(not(feature = "network"))]
            {
                let _ = env;
                panic!("Postgres URL feature is not enabled");
            }
        };
        let executor = DatabaseExecutor { database_client };

        Ok(Box::new(PostgresSubsystemResolver {
            id: self.id(),
            subsystem,
            executor,
        }))
    }
}
