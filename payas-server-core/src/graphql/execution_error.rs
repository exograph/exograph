use std::error::Error;

use payas_resolver_database::DatabaseExecutionError;
use thiserror::Error;

use crate::graphql::validation::validation_error::ValidationError;

use payas_resolver_deno::DenoExecutionError;

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
}

impl ExecutionError {
    // Message that should be emitted when the error is returned to the user.
    // This should hide any internal details of the error.
    // TODO: Log the details of the error.
    pub fn user_error_message(&self) -> String {
        match self {
            ExecutionError::Database(error) => error.user_error_message(),
            ExecutionError::Deno(DenoExecutionError::Delegate(error)) => {
                match error.downcast_ref::<ExecutionError>() {
                    Some(error) => error.user_error_message(),
                    None => error.to_string(),
                }
            }
            _ => match self.source() {
                Some(source) => source.to_string(),
                None => self.to_string(),
            },
        }
    }
}
