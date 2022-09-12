use codemap_diagnostic::Diagnostic;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModelBuildingError {
    #[error("Could not process input clay files")]
    Diagnosis(Vec<Diagnostic>),

    #[error("{0}")]
    IO(#[from] std::io::Error),

    #[error("{0}")]
    Generic(String),
}
