use codemap_diagnostic::Diagnostic;
use core_plugin::error::ModelSerializationError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModelBuildingError {
    #[error("Could not process input clay files")]
    Diagnosis(Vec<Diagnostic>),

    #[error("{0}")]
    IO(#[from] std::io::Error),

    #[error("{0}")]
    Generic(String),

    #[error("Unable to serialize model {0}")]
    Serialize(#[source] ModelSerializationError),

    #[error("Unable to deserialize model {0}")]
    Deserialize(#[source] ModelSerializationError),
}
