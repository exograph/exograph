use std::error::Error;

use payas_resolver_database::DatabaseExecutionError;
use payas_resolver_wasm::WasmExecutionError;
use thiserror::Error;

use crate::graphql::validation::validation_error::ValidationError;

use payas_resolver_deno::DenoExecutionError;

#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("{0}")]
    Generic(String),

    #[error("{0}")]
    Database(#[from] DatabaseExecutionError),

    #[error("{0}")]
    Deno(#[from] DenoExecutionError),

    #[error("{0}")]
    Wasm(#[from] WasmExecutionError),

    #[error("{0}")]
    Serde(#[from] serde_json::Error),

    #[error("{0}")]
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
            ExecutionError::Deno(error) => match error {
                DenoExecutionError::Delegate(underlying) => {
                    match underlying.downcast_ref::<ExecutionError>() {
                        Some(error) => error.user_error_message(),
                        None => error.user_error_message(),
                    }
                }
                _ => error.user_error_message(),
            },
            _ => match self.source() {
                Some(source) => source.to_string(),
                None => self.to_string(),
            },
        }
    }
}
