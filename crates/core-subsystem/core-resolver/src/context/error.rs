use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContextParsingError {
    #[error("Could not find source `{0}`")]
    SourceNotFound(String),

    #[error("Unauthorized request")]
    Unauthorized,

    #[error("Malformed request")]
    Malformed,

    #[error("{0}")]
    Delegate(#[from] Box<dyn std::error::Error + Send + Sync>),
}
