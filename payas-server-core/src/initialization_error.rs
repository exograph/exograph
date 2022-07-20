use thiserror::Error;

#[derive(Error, Debug)]
pub enum InitializationError {
    #[error("{0}")]
    Generic(String),

    #[error("No such file {0}")]
    FileNotFound(String),

    #[error("Failed to open file {0}")]
    FileOpen(String, #[source] std::io::Error),

    #[error("Invalid claypot file {0}")]
    ClaypotDeserialization(String, #[source] bincode::Error),
}
