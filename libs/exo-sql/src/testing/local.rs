//! Ephemeral database server based on a local postgres installation

use std::{
    fs::OpenOptions,
    io::Write,
    process::{Child, Stdio},
};

use tempfile::TempDir;

use super::{
    db::{launch_process, EphemeralDatabase, EphemeralDatabaseServer},
    error::EphemeralDatabaseSetupError,
};

pub struct LocalPostgresDatabaseServer {
    process: Child,
    data_dir: TempDir,
}

pub struct LocalPostgresDatabase {
    data_dir: String,
    name: String,
}

impl LocalPostgresDatabaseServer {
    pub fn check_availability() -> Result<(), EphemeralDatabaseSetupError> {
        which::which("initdb")?;
        which::which("postgres")?;
        which::which("pg_isready")?;
        which::which("createdb")?;
        Ok(())
    }

    pub fn start(
    ) -> Result<Box<dyn EphemeralDatabaseServer + Send + Sync>, EphemeralDatabaseSetupError> {
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
        )?;

        let config_file = data_dir.path().join("postgresql.conf");
        let mut file = OpenOptions::new()
            .append(true)
            .open(&config_file)
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

        let postgres = std::process::Command::new("postgres")
            .args([
                "-D",
                data_dir.path().to_str().unwrap(),
                "-k",
                data_dir.path().to_str().unwrap(),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
                EphemeralDatabaseSetupError::Generic(format!("Failed to start Postgres: {e}"))
            })?;

        let mut tries = 0;
        loop {
            let result = launch_process(
                "pg_isready",
                &["-h", data_dir.path().to_str().unwrap(), "-U", "exo"],
            );

            if result.is_ok() {
                break;
            }

            tries += 1;
            if tries > 1000 {
                return Err(EphemeralDatabaseSetupError::Generic(
                    "Postgres failed to start".into(),
                ));
            }

            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        Ok(Box::new(LocalPostgresDatabaseServer {
            process: postgres,
            data_dir,
        }))
    }
}

impl EphemeralDatabaseServer for LocalPostgresDatabaseServer {
    fn create_database(
        &self,
        name: &str,
    ) -> Result<Box<dyn EphemeralDatabase + Send + Sync>, EphemeralDatabaseSetupError> {
        launch_process(
            "createdb",
            &[
                "-h",
                self.data_dir.path().to_str().unwrap(),
                "-U",
                "exo",
                name,
            ],
        )?;

        Ok(Box::new(LocalPostgresDatabase {
            data_dir: self.data_dir.path().to_str().unwrap().into(),
            name: name.into(),
        }))
    }
}

impl Drop for LocalPostgresDatabaseServer {
    fn drop(&mut self) {
        std::fs::remove_dir_all(self.data_dir.path()).unwrap();
        let _ = self.process.kill();
    }
}

impl EphemeralDatabase for LocalPostgresDatabase {
    fn url(&self) -> String {
        format!(
            "postgres://exo@{}/{}",
            urlencoding::encode(&self.data_dir),
            self.name
        )
    }
}

impl Drop for LocalPostgresDatabase {
    fn drop(&mut self) {
        launch_process(
            "dropdb",
            &["-h", &self.data_dir, "--force", "-U", "exo", &self.name],
        )
        .unwrap()
    }
}
