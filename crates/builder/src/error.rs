use codemap_diagnostic::Diagnostic;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("Could not process input clay filesx {:?}", .0)]
    Diagnosis(Vec<Diagnostic>),

    #[error("File '{0}' not found")]
    FileNotFound(String),

    #[error("{0}")]
    IO(#[from] std::io::Error),

    #[error("{0}")]
    ModelBuildingError(#[from] core_model_builder::error::ModelBuildingError),

    #[error("{0}")]
    ModelSerializationError(#[from] core_plugin::error::ModelSerializationError),

    #[error("{0}")]
    Generic(String),
}
