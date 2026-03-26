// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Ephemeral database server that uses an already-running Postgres instance.
//! Instead of running `initdb` and `pg_ctl` to create a new cluster (like the local mode), this mode
//! connects to an existing Postgres and uses `createdb`/`dropdb` to manage
//! ephemeral databases.

use super::{
    db::{EphemeralDatabase, EphemeralDatabaseServer, launch_process},
    error::EphemeralDatabaseSetupError,
};

/// Environment variable to specify the connection URL for the existing Postgres instance.
/// If not set, defaults to the libpq defaults (typically Unix socket with current OS user).
const EXO_SQL_EXISTING_DB_URL: &str = "EXO_SQL_EXISTING_DB_URL";

pub struct ExistingPostgresDatabaseServer {
    base_url: url::Url,
}

pub struct ExistingPostgresDatabase {
    base_url: url::Url,
    name: String,
}

impl ExistingPostgresDatabaseServer {
    pub fn create_server()
    -> Result<Box<dyn EphemeralDatabaseServer + Send + Sync>, EphemeralDatabaseSetupError> {
        let available = Self::check_availability();
        if let Ok(true) = available {
            tracing::info!("Connecting to existing PostgreSQL server...");
            Self::start()
        } else {
            tracing::error!("Existing PostgreSQL server is not available");
            Err(EphemeralDatabaseSetupError::Generic(
                "Existing PostgreSQL server is not available".to_string(),
            ))
        }
    }

    pub fn check_availability() -> Result<bool, EphemeralDatabaseSetupError> {
        if let Err(e) = which::which("createdb") {
            tracing::error!("createdb not found: {}", e);
            return Ok(false);
        }
        if let Err(e) = which::which("dropdb") {
            tracing::error!("dropdb not found: {}", e);
            return Ok(false);
        }
        if let Err(e) = which::which("pg_isready") {
            tracing::error!("pg_isready not found: {}", e);
            return Ok(false);
        }
        Ok(true)
    }

    pub fn start()
    -> Result<Box<dyn EphemeralDatabaseServer + Send + Sync>, EphemeralDatabaseSetupError> {
        let base_url = match std::env::var(EXO_SQL_EXISTING_DB_URL) {
            Ok(url_str) => {
                let mut url = url::Url::parse(&url_str).map_err(|e| {
                    EphemeralDatabaseSetupError::Generic(format!(
                        "Invalid {EXO_SQL_EXISTING_DB_URL} URL: {e}"
                    ))
                })?;
                url.set_path("");
                url
            }
            Err(_) => url::Url::parse("postgres://localhost/")
                .expect("Hardcoded default URL should always be valid"),
        };

        let url_string = base_url.to_string();
        launch_process("pg_isready", &["-d", &url_string], true).map_err(|_| {
            EphemeralDatabaseSetupError::Generic(
                "Existing PostgreSQL is not reachable. Ensure it is running and accessible."
                    .to_string(),
            )
        })?;

        tracing::info!("Connected to existing PostgreSQL");

        Ok(Box::new(ExistingPostgresDatabaseServer { base_url }))
    }

    fn url_with_path(base_url: &url::Url, path: &str) -> String {
        let mut url = base_url.clone();
        url.set_path(path);
        url.to_string()
    }
}

impl EphemeralDatabaseServer for ExistingPostgresDatabaseServer {
    fn create_database(
        &self,
        name: &str,
    ) -> Result<Box<dyn EphemeralDatabase + Send + Sync>, EphemeralDatabaseSetupError> {
        let maintenance_url = Self::url_with_path(&self.base_url, "/postgres");
        launch_process(
            "createdb",
            &["--maintenance-db", &maintenance_url, name],
            true,
        )?;

        Ok(Box::new(ExistingPostgresDatabase {
            base_url: self.base_url.clone(),
            name: name.to_string(),
        }))
    }

    fn cleanup(&self) {}
}

impl EphemeralDatabase for ExistingPostgresDatabase {
    fn url(&self) -> String {
        ExistingPostgresDatabaseServer::url_with_path(&self.base_url, &format!("/{}", self.name))
    }
}

impl Drop for ExistingPostgresDatabase {
    fn drop(&mut self) {
        let maintenance_url =
            ExistingPostgresDatabaseServer::url_with_path(&self.base_url, "/postgres");
        launch_process(
            "dropdb",
            &["--maintenance-db", &maintenance_url, "--force", &self.name],
            false,
        )
        .unwrap_or_else(|e| {
            tracing::error!("Failed to drop database '{}': {}", self.name, e);
        });
    }
}
