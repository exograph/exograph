use payas_core_plugin::error::ModelSerializationError;
use thiserror::Error;

use super::subsystem_resolver::SubsystemResolver;

pub trait SubsystemLoader {
    fn id(&self) -> &'static str;

    fn init(
        &self,
        serialized_subsystem: Vec<u8>,
    ) -> Result<Box<dyn SubsystemResolver + Send + Sync>, SubsystemLoadingError>;
}

#[derive(Error, Debug)]
pub enum SubsystemLoadingError {
    #[error("System serialization error: {0}")]
    ModelSerializationError(#[from] ModelSerializationError),

    #[error("{0}")]
    BoxedError(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}
