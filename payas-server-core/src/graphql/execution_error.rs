use std::error::Error;

use thiserror::Error;

use crate::graphql::validation::validation_error::ValidationError;

use super::data::{database::DatabaseExecutionError, deno::DenoExecutionError};

#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("{0}")]
    Generic(String),

    #[error(transparent)]
    Database(#[from] DatabaseExecutionError),

    #[error(transparent)]
    Deno(#[from] DenoExecutionError),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error(transparent)]
    Validation(#[from] ValidationError),

    #[error("Invalid field {0} for {1}")]
    InvalidField(String, &'static str), // (field name, container type)

    #[error("Not authorized")]
    Authorization,

    #[error("{0} {1}")]
    WithContext(String, #[source] Box<ExecutionError>),
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
            // Do not reveal the underlying database error as it may expose sensitive details (such as column names or data involved in constraint violation).
            ExecutionError::Database(_error) => "Database operation failed".to_string(),
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
