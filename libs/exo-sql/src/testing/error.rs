use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum EphemeralDatabaseSetupError {
    #[error("Failed to start postgres")]
    PostgresFailedToStart(#[from] io::Error),
    #[error("Failed to find executable")]
    ExecutableNotFound(#[from] which::Error),

    #[error("Failed to start Docker (it may not be installed) {0}")]
    Docker(#[source] io::Error),

    #[error("Failed to start postgres {0}")]
    Generic(String),
}
