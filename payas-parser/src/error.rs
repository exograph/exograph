use codemap_diagnostic::Diagnostic;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("Could not process input clay files")]
    Diagnosis(Vec<Diagnostic>),

    #[error("File '{0}' not found")]
    FileNotFound(String),

    #[error("{0}")]
    IO(#[from] std::io::Error),

    #[error("{0}")]
    ModelBuildingError(#[from] payas_core_model_builder::error::ModelBuildingError),

    #[error("{0}")]
    Generic(String),
}
