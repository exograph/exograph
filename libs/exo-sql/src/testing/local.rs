// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Ephemeral database server based on a local postgres installation

#[cfg(not(unix))]
use std::net::TcpListener;

#[cfg(unix)]
use std::{fs::OpenOptions, io::Write};

use std::process::Stdio;
use tempfile::TempDir;

use super::{
    db::{EphemeralDatabase, EphemeralDatabaseServer, launch_process},
    error::EphemeralDatabaseSetupError,
};

pub struct LocalPostgresDatabaseServer {
    port: Option<u16>,
    data_dir: TempDir,
}

pub struct LocalPostgresDatabase {
    #[allow(unused)]
    data_dir: String,
    #[allow(unused)]
    port: Option<u16>,
    name: String,
}

impl LocalPostgresDatabaseServer {
    pub fn check_availability() -> Result<bool, EphemeralDatabaseSetupError> {
        if let Err(e) = which::which("initdb") {
            tracing::error!("initdb not found: {}", e);
            return Ok(false);
        }
        if let Err(e) = which::which("pg_ctl") {
            tracing::error!("pg_ctl not found: {}", e);
            return Ok(false);
        }
        if let Err(e) = which::which("pg_isready") {
            tracing::error!("pg_isready not found: {}", e);
            return Ok(false);
        }
        if let Err(e) = which::which("createdb") {
            tracing::error!("createdb not found: {}", e);
            return Ok(false);
        }
        Ok(true)
    }

    pub fn start()
    -> Result<Box<dyn EphemeralDatabaseServer + Send + Sync>, EphemeralDatabaseSetupError> {
        let data_dir = tempfile::tempdir().map_err(|e| {
            EphemeralDatabaseSetupError::Generic(format!(
                "Failed to create temporary directory: {e}",
            ))
        })?;

        launch_process(
            "initdb",
            &[
                "-D",
                data_dir.path().to_str().unwrap(),
                "-A",
                "trust",
                "--username",
                "exo",
            ],
            true,
        )?;

        #[cfg(unix)]
        {
            let config_file = data_dir.path().join("postgresql.conf");
            let mut file = OpenOptions::new()
                .append(true)
                .open(config_file)
                .map_err(|e| {
                    EphemeralDatabaseSetupError::Generic(format!(
                        "Failed to open Postgres config file: {e}"
                    ))
                })?;

            file.write_all(b"\nlisten_addresses = ''\n").map_err(|e| {
                EphemeralDatabaseSetupError::Generic(format!(
                    "Failed to write to Postgres config file: {e}"
                ))
            })?;
            drop(file);
        }

        let mut postgres = std::process::Command::new("pg_ctl");
        postgres.args(["start", "-D", data_dir.path().to_str().unwrap()]);

        #[cfg(unix)]
        {
            postgres.args(["-o", &format!("-k {}", data_dir.path().to_str().unwrap())]);
        }

        let port: Option<u16> = {
            #[cfg(unix)]
            {
                None
            }

            #[cfg(not(unix))]
            {
                let temp_listener = TcpListener::bind("127.0.0.1:0").unwrap();
                Some(temp_listener.local_addr().unwrap().port())
            }
        };

        #[cfg(not(unix))]
        {
            postgres.args([
                "-o",
                &format!(
                    "-h localhost -p {}",
                    port.as_ref().unwrap().to_string().as_str()
                ),
            ]);
        }

        postgres
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                EphemeralDatabaseSetupError::Generic(format!("Failed to start Postgres: {e}"))
            })?;

        let mut tries = 0;
        loop {
            #[cfg(not(unix))]
            let port_string = port.map(|p| format!("{}", p));
            let args = {
                #[cfg(unix)]
                {
                    vec!["-h", data_dir.path().to_str().unwrap(), "-U", "exo"]
                }

                #[cfg(not(unix))]
                {
                    vec![
                        "-h",
                        "localhost",
                        "-p",
                        port_string.as_ref().unwrap().as_str(),
                        "-U",
                        "exo",
                    ]
                }
            };

            let result = launch_process("pg_isready", &args, true);

            if result.is_ok() {
                break;
            }

            tries += 1;
            if tries > 10 {
                return Err(EphemeralDatabaseSetupError::Generic(
                    "Postgres failed to start".into(),
                ));
            }

            std::thread::sleep(std::time::Duration::from_millis(100));
            eprintln!("Waiting for Postgres to start...");
        }

        Ok(Box::new(LocalPostgresDatabaseServer { port, data_dir }))
    }
}

impl EphemeralDatabaseServer for LocalPostgresDatabaseServer {
    fn create_database(
        &self,
        name: &str,
    ) -> Result<Box<dyn EphemeralDatabase + Send + Sync>, EphemeralDatabaseSetupError> {
        #[cfg(not(unix))]
        let port_string = self.port.map(|p| format!("{}", p));

        let args = {
            #[cfg(unix)]
            {
                vec![
                    "-h",
                    self.data_dir.path().to_str().unwrap(),
                    "-U",
                    "exo",
                    name,
                ]
            }

            #[cfg(not(unix))]
            {
                vec![
                    "-h",
                    "localhost",
                    "-p",
                    port_string.as_ref().unwrap().as_str(),
                    "-U",
                    "exo",
                    name,
                ]
            }
        };

        launch_process("createdb", &args, true)?;

        Ok(Box::new(LocalPostgresDatabase {
            data_dir: self.data_dir.path().to_str().unwrap().into(),
            port: self.port,
            name: name.into(),
        }))
    }

    fn cleanup(&self) {
        std::process::Command::new("pg_ctl")
            .args(["stop", "-D", self.data_dir.path().to_str().unwrap()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
            .wait()
            .unwrap();

        std::fs::remove_dir_all(self.data_dir.path()).unwrap();
    }
}

impl Drop for LocalPostgresDatabaseServer {
    fn drop(&mut self) {
        self.cleanup();
    }
}

impl EphemeralDatabase for LocalPostgresDatabase {
    fn url(&self) -> String {
        #[cfg(unix)]
        {
            format!(
                "postgres://exo@{}/{}",
                urlencoding::encode(&self.data_dir),
                self.name
            )
        }

        #[cfg(not(unix))]
        {
            format!(
                "postgres://exo@localhost:{}/{}",
                self.port.unwrap(),
                self.name
            )
        }
    }
}

impl Drop for LocalPostgresDatabase {
    fn drop(&mut self) {
        #[cfg(not(unix))]
        let port_string = self.port.map(|p| format!("{}", p));

        let args = {
            #[cfg(unix)]
            {
                vec![
                    "-h",
                    &self.data_dir,
                    "--force",
                    "--username",
                    "exo",
                    &self.name,
                ]
            }

            #[cfg(not(unix))]
            {
                vec![
                    "-h",
                    "localhost",
                    "-p",
                    port_string.as_ref().unwrap().as_str(),
                    "--force",
                    "--username",
                    "exo",
                    &self.name,
                ]
            }
        };

        launch_process("dropdb", &args, false).unwrap_or(()); // Ignore errors
    }
}
