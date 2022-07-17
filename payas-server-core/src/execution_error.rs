use std::error::Error;

use payas_deno::deno_error::DenoError;
use thiserror::Error;

use crate::validation_error::ValidationError;

#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("{0}")]
    Generic(String),

    #[error("Result has {0} entries; expected only zero or one")]
    NonUniqueResult(usize),

    #[error("Invalid argument {0}")]
    InvalidServiceArgument(String),

    #[error(transparent)]
    EmptyRow(#[from] tokio_postgres::Error),

    #[error(transparent)]
    Deno(#[from] DenoError),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error(transparent)]
    Validation(#[from] ValidationError),

    #[error(transparent)]
    AnyhowError(#[from] anyhow::Error),

    #[error("Invalid field {0} for {1}")]
    InvalidField(String, &'static str), // (field name, container type)

    #[error("Not authorized")]
    Authorization,

    #[error("{0} {1}")]
    WithContext(String, #[source] Box<ExecutionError>),

    #[error("{0}")]
    InvalidLiteral(String, #[source] anyhow::Error),
}

impl ExecutionError {
    pub fn with_context(self, context: String) -> ExecutionError {
        ExecutionError::WithContext(context, Box::new(self))
    }

    // Message that should be emitted when the error is returned to the user.
    // This should hide any internal details of the error.
    // TODO: Log the details of the error.
    pub fn user_error_message(&self) -> String {
        match self {
            ExecutionError::WithContext(_message, source) => source.user_error_message(),
            ExecutionError::AnyhowError(error) => error.to_string(),
            _ => match self.source() {
                Some(source) => source.to_string(),
                None => self.to_string(),
            },
        }
    }
}

pub trait WithContext {
    fn with_context(self, context: String) -> Self;
}

impl<T> WithContext for Result<T, ExecutionError> {
    fn with_context(self, context: String) -> Result<T, ExecutionError> {
        self.map_err(|e| e.with_context(context))
    }
}
