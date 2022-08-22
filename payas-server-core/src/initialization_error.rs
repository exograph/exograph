use payas_sql::database_error::DatabaseError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum InitializationError {
    #[error("No such file {0}")]
    FileNotFound(String),

    #[error("{0}")]
    Database(#[from] DatabaseError),

    #[error("Failed to open file {0}")]
    FileOpen(String, #[source] std::io::Error),

    #[error("Invalid claypot file {0}")]
    ClaypotDeserialization(String, #[source] bincode::Error),

    #[error("Configuration error: {0}")]
    Config(String),
}
