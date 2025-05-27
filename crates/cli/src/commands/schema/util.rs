// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::anyhow;

use core_plugin_shared::{
    serializable_system::SerializableSystem, system_serializer::SystemSerializer,
};
use exo_sql::Database;
use exo_sql::{database_error::DatabaseError, DatabaseClientManager};
use postgres_core_model::subsystem::PostgresCoreSubsystem;

use crate::commands::build::build_system_with_static_builders;
use crate::commands::command::ensure_exo_project_dir;
use common::env_const::{
    DATABASE_URL, EXO_CHECK_CONNECTION_ON_STARTUP, EXO_CONNECTION_POOL_SIZE, EXO_POSTGRES_URL,
};

pub(crate) async fn open_database(
    database: Option<&str>,
) -> Result<DatabaseClientManager, DatabaseError> {
    if let Some(database) = database {
        Ok(DatabaseClientManager::from_url(database, true, None).await?)
    } else {
        Ok(database_manager_from_env().await?)
    }
}

pub(crate) async fn database_manager_from_env() -> Result<DatabaseClientManager, DatabaseError> {
    let url = std::env::var(EXO_POSTGRES_URL)
        .or(std::env::var(DATABASE_URL))
        .or(Err(DatabaseError::Config(format!(
            "{EXO_POSTGRES_URL} or {DATABASE_URL} not set"
        ))))?;
    let pool_size = std::env::var(EXO_CONNECTION_POOL_SIZE)
        .ok()
        .and_then(|s| s.parse().ok());
    let check_connection = std::env::var(EXO_CHECK_CONNECTION_ON_STARTUP)
        .ok()
        .map(|s| s == "true")
        .unwrap_or(true);

    DatabaseClientManager::from_url(&url, check_connection, pool_size).await
}

pub(crate) async fn create_system(
    model_file: impl AsRef<Path>,
    trusted_documents_dir: Option<&Path>,
    use_ir: bool,
) -> Result<SerializableSystem, anyhow::Error> {
    if use_ir {
        let exo_ir_file = PathBuf::from("target/index.exo_ir");
        if !Path::new(&exo_ir_file).exists() {
            return Err(anyhow!("IR file not found"));
        }

        match File::open(exo_ir_file) {
            Ok(file) => {
                let exo_ir_file_buffer = BufReader::new(file);

                SerializableSystem::deserialize_reader(exo_ir_file_buffer)
                    .map_err(|e| anyhow!("Error deserializing system: {:?}", e))
            }
            Err(e) => Err(anyhow!("Error opening IR file: {}", e)),
        }
    } else {
        ensure_exo_project_dir(&PathBuf::from("."))?;
        Ok(
            build_system_with_static_builders(model_file.as_ref(), trusted_documents_dir, None)
                .await?,
        )
    }
}

pub(crate) async fn extract_postgres_database(
    model_file: impl AsRef<Path>,
    trusted_documents_dir: Option<&Path>,
    use_ir: bool,
) -> Result<Database, anyhow::Error> {
    let serialized_system = create_system(model_file, trusted_documents_dir, use_ir).await?;

    let postgres_subsystem = serialized_system
        .subsystems
        .into_iter()
        .find(|subsystem| subsystem.id == "postgres");

    let database = match postgres_subsystem {
        Some(subsystem) => PostgresCoreSubsystem::deserialize(subsystem.core.0)?.database,
        None => Database::default(),
    };

    Ok(database)
}
