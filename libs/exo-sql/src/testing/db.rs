use super::{
    docker::DockerPostgresDatabaseServer, error::EphemeralDatabaseSetupError,
    local::LocalPostgresDatabaseServer,
};

/// Launcher for an ephemeral database server using either a local Postgres installation or Docker
pub struct EphemeralDatabaseLauncher {}

impl EphemeralDatabaseLauncher {
    /// Create a new database server.
    /// Currently, it prefers a local installation, but falls back to Docker if it's not available
    pub fn create_server(
    ) -> Result<Box<dyn EphemeralDatabaseServer + Send + Sync>, EphemeralDatabaseSetupError> {
        if LocalPostgresDatabaseServer::check_availability().is_ok() {
            println!("Launching PostgreSQL locally...");
            LocalPostgresDatabaseServer::start()
        } else if DockerPostgresDatabaseServer::check_availability().is_ok() {
            println!("Launching PostgreSQL in Docker...");
            DockerPostgresDatabaseServer::start()
        } else {
            Err(EphemeralDatabaseSetupError::Generic(
                "Neither local PostgreSQL nor Docker is available".to_string(),
            ))
        }
    }
}

/// A ephemeral database server that can create ephemeral databases
/// Implemented should implement `Drop` to clean up the server to free up resources
pub trait EphemeralDatabaseServer {
    /// Create a new database on the server with the specified name
    fn create_database(
        &self,
        name: &str,
    ) -> Result<Box<dyn EphemeralDatabase + Send + Sync>, EphemeralDatabaseSetupError>;
}

/// A ephemeral database that can be used for testing.
/// Implemented should implement `Drop` to clean up the database to free up resources
pub trait EphemeralDatabase {
    /// Get the URL to connect to the database. The URL should be in the format suitable as the `psql` argument
    fn url(&self) -> String;
}

/// A utility function to launch a process and wait for it to exit
pub(super) fn launch_process(name: &str, args: &[&str]) -> Result<(), EphemeralDatabaseSetupError> {
    let mut command = std::process::Command::new(name);
    let command = command.args(args);
    let command = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped());

    let mut command = command.spawn().map_err(|e| {
        EphemeralDatabaseSetupError::Generic(format!("Failed to launch process {}: {}", name, e))
    })?;

    let status = command.wait().map_err(|e| {
        EphemeralDatabaseSetupError::Generic(format!("Failed to wait for process {}: {}", name, e))
    })?;

    if !status.success() {
        return Err(EphemeralDatabaseSetupError::Generic(format!(
            "Process {} exited with non-zero status code {}",
            name, status,
        )));
    }
    Ok(())
}
