// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! A docker implementation of an ephemeral database server

use rand::Rng;
use std::{
    io::{BufRead, BufReader},
    process::Stdio,
};

use super::{
    db::{launch_process, EphemeralDatabase, EphemeralDatabaseServer},
    error::EphemeralDatabaseSetupError,
};

pub struct DockerPostgresDatabaseServer {
    container_name: String,
    port: u16,
}

pub struct DockerPostgresDatabase {
    container_name: String,
    port: u16,
    name: String,
}

impl DockerPostgresDatabaseServer {
    pub fn check_availability() -> Result<(), EphemeralDatabaseSetupError> {
        which::which("docker")
            .map_err(|e| EphemeralDatabaseSetupError::ExecutableNotFound("docker", e))?;
        Ok(())
    }

    pub fn start(
    ) -> Result<Box<dyn EphemeralDatabaseServer + Send + Sync>, EphemeralDatabaseSetupError> {
        // acquire an empty port
        let port = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| {
                EphemeralDatabaseSetupError::Generic(format!("Failed to acquire an empty port {e}"))
            })?;
            let addr = listener.local_addr()?;
            addr.port()
        };

        // generate container name
        let container_name = format!("exograph-db-{}", generate_random_string());

        // start postgres docker in background
        let mut db_background = std::process::Command::new("docker");
        let db_background = db_background
            .args([
                "run",
                "--rm",
                "--name",
                &container_name,
                "-e",
                "POSTGRES_USER=exo",
                "-e",
                "POSTGRES_PASSWORD=exo",
                "-p",
                &format!("{port}:5432"),
                "postgres",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut db_background = db_background
            .spawn()
            .map_err(EphemeralDatabaseSetupError::Docker)?;

        // let things stabilize

        let stderr = db_background.stderr.take().unwrap();
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            let line = line?;
            if line.contains("database system is ready to accept connections") {
                break;
            }
        }

        Ok(Box::new(DockerPostgresDatabaseServer {
            container_name,
            port,
        }))
    }
}

impl EphemeralDatabaseServer for DockerPostgresDatabaseServer {
    fn create_database(
        &self,
        name: &str,
    ) -> Result<Box<dyn EphemeralDatabase + Send + Sync>, EphemeralDatabaseSetupError> {
        launch_process(
            "docker",
            &["exec", &self.container_name, "createdb", "-U", "exo", name],
            true,
        )?;

        Ok(Box::new(DockerPostgresDatabase {
            container_name: self.container_name.clone(),
            port: self.port,
            name: name.into(),
        }))
    }
}

impl Drop for DockerPostgresDatabaseServer {
    fn drop(&mut self) {
        // kill docker, will get removed automatically on exit due to --rm provided when starting
        if let Err(e) = launch_process("docker", &["kill", &self.container_name], false) {
            eprintln!(
                "Failed to kill docker container '{}': {}",
                self.container_name, e
            );
        }
    }
}

impl EphemeralDatabase for DockerPostgresDatabase {
    fn url(&self) -> String {
        format!(
            "postgresql://exo:exo@127.0.0.1:{}/{}?sslmode=disable",
            self.port, self.name
        )
    }
}

impl Drop for DockerPostgresDatabase {
    fn drop(&mut self) {
        if let Err(e) = launch_process(
            "docker",
            &[
                "exec",
                &self.container_name,
                "dropdb",
                "-U",
                "exo",
                &self.name,
            ],
            false,
        ) {
            eprintln!(
                "Failed to drop database '{}' in container '{}': {}",
                self.name, self.container_name, e
            );
        }
    }
}

fn generate_random_string() -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(15)
        .map(char::from)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}
