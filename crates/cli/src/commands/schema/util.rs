// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::path::Path;

use builder::error::ParserError;
use core_plugin_shared::{
    serializable_system::SerializableSystem, system_serializer::SystemSerializer,
};
use exo_sql::{database_error::DatabaseError, DatabaseClientManager};
use postgres_model::subsystem::PostgresSubsystem;

use crate::commands::build::build_system_with_static_builders;

pub(crate) async fn create_postgres_system(
    model_file: impl AsRef<Path>,
    trusted_documents_dir: Option<&Path>,
) -> Result<PostgresSubsystem, ParserError> {
    deserialize_postgres_subsystem(
        build_system_with_static_builders(model_file.as_ref(), trusted_documents_dir).await?,
    )
}

fn deserialize_postgres_subsystem(
    system: SerializableSystem,
) -> Result<PostgresSubsystem, ParserError> {
    system
        .subsystems
        .into_iter()
        .find_map(|subsystem| {
            if subsystem.id == "postgres" {
                Some(PostgresSubsystem::deserialize(
                    subsystem.serialized_subsystem,
                ))
            } else {
                None
            }
        })
        // If there is no database subsystem in the serialized system, create an empty one
        .unwrap_or_else(|| Ok(PostgresSubsystem::default()))
        .map_err(|e| {
            ParserError::Generic(format!("Error while deserializing database subsystem: {e}"))
        })
}

pub(crate) async fn database_manager_from_env() -> Result<DatabaseClientManager, DatabaseError> {
    let url = std::env::var("EXO_POSTGRES_URL").expect("EXO_POSTGRES_URL not set");
    let user = std::env::var("EXO_POSTGRES_USER").ok();
    let password = std::env::var("EXO_POSTGRES_PASSWORD").ok();
    let pool_size = std::env::var("EXO_CONNECTION_POOL_SIZE")
        .ok()
        .and_then(|s| s.parse().ok());
    let check_connection = std::env::var("EXO_CHECK_CONNECTION_ON_STARTUP")
        .ok()
        .map(|s| s == "true")
        .unwrap_or(true);

    DatabaseClientManager::from_url(&url, &user, &password, check_connection, pool_size).await
}
