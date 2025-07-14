use thiserror::Error;

use exo_env::EnvError;

use core_plugin_interface::interface::{LibraryLoadingError, SubsystemLoadingError};
use core_plugin_shared::error::ModelSerializationError;

#[derive(Error, Debug)]
pub enum SystemLoadingError {
    #[error("System serialization error: {0}")]
    ModelSerializationError(#[from] ModelSerializationError),

    #[error("Error while trying to load subsystem library: {0}")]
    LibraryLoadingError(#[from] LibraryLoadingError),

    #[error("Subsystem loading error: {0}")]
    SubsystemLoadingError(#[from] SubsystemLoadingError),

    #[error("No such file {0}")]
    FileNotFound(String),

    #[error("Failed to open file {0}")]
    FileOpen(String, #[source] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("{0}")]
    EnvError(#[from] EnvError),
}
