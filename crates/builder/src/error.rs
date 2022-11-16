use codemap_diagnostic::Diagnostic;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParserError {
    // Don't include the source, because we emit is as a diagnostic
    #[error("Could not process input clay files")]
    Diagnosis(Vec<Diagnostic>),

    #[error("File '{0}' not found")]
    FileNotFound(String),

    #[error("{0}")]
    IO(#[from] std::io::Error),

    #[error("{0}")]
    ModelBuildingError(#[from] core_model_builder::error::ModelBuildingError),

    #[error("{0}")]
    ModelSerializationError(#[from] core_plugin_shared::error::ModelSerializationError),

    #[error("{0}")]
    Generic(String),
}
