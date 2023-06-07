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
use exo_sql::{DatabaseClient, DatabaseExecutor};
use postgres_model::subsystem::PostgresSubsystem;

pub struct PostgresSubsystemLoader {}

#[async_trait]
impl SubsystemLoader for PostgresSubsystemLoader {
    fn id(&self) -> &'static str {
        "postgres"
    }

    async fn init(
        &self,
        serialized_subsystem: Vec<u8>,
    ) -> Result<Box<dyn SubsystemResolver + Send + Sync>, SubsystemLoadingError> {
        let subsystem = PostgresSubsystem::deserialize(serialized_subsystem)?;

        let database_client = DatabaseClient::from_env(None)
            .await
            .map_err(|e| SubsystemLoadingError::BoxedError(Box::new(e)))?;
        let executor = DatabaseExecutor { database_client };

        Ok(Box::new(PostgresSubsystemResolver {
            id: self.id(),
            subsystem,
            executor,
        }))
    }
}
