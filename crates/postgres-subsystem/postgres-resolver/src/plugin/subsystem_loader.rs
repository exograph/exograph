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

use core_plugin_interface::{
    core_resolver::plugin::SubsystemResolver,
    interface::{SubsystemLoader, SubsystemLoadingError},
    system_serializer::SystemSerializer,
};
use exo_env::Environment;
use exo_sql::DatabaseClientManager;
use postgres_core_resolver::create_database_executor;
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
        serialized_subsystem: (Option<Vec<u8>>, Option<Vec<u8>>),
        env: &dyn Environment,
    ) -> Result<Box<dyn SubsystemResolver + Send + Sync>, SubsystemLoadingError> {
        let subsystem = PostgresSubsystem::deserialize(serialized_subsystem.0.unwrap())?;

        let executor = create_database_executor(self.existing_client.take(), env)
            .await
            .map_err(|e| SubsystemLoadingError::BoxedError(Box::new(e)))?;

        Ok(Box::new(PostgresSubsystemResolver {
            id: self.id(),
            subsystem,
            executor,
        }))
    }
}
