use payas_core_resolver::plugin::{SubsystemResolutionError, SystemResolutionError};
use thiserror::Error;

use payas_deno::deno_error::DenoError;

#[derive(Error, Debug)]
pub enum DenoExecutionError {
    #[error("{0}")]
    Deno(#[source] DenoError),

    #[error("Invalid argument {0}")]
    InvalidArgument(String),

    #[error("Not authorized")]
    Authorization,

    #[error("{0}")]
    Generic(String),

    #[error("{0}")]
    Delegate(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl DenoExecutionError {
    pub fn user_error_message(&self) -> String {
        match self {
            DenoExecutionError::Authorization => "Not authorized".to_string(),
            DenoExecutionError::Deno(DenoError::Explicit(error)) => error.to_string(),
            DenoExecutionError::Delegate(error) => {
                match error.downcast_ref::<DenoExecutionError>() {
                    Some(error) => error.user_error_message(),
                    None => "Internal server error".to_string(),
                }
            }
            _ => "Internal server error".to_string(), // Do not reveal too much information about the error
        }
    }

    pub fn explicit_message(&self) -> Option<String> {
        fn root_error<'a>(
            error: &'a (dyn std::error::Error + 'static),
        ) -> &'a (dyn std::error::Error + 'static) {
            match error.source() {
                Some(source) => root_error(source),
                None => error,
            }
        }

        let root_error = root_error(self);

        // To deal with nested exceptions, we need to check if it is an explicit error (direct
        // invocation, so DenoError::Explicit suffices), or did it invoke it in nested fashion
        // (indirect invocation, SubsystemResolutionError), or it invoked another subsystem (cross
        // invocation, SystemResolutionError).
        match root_error.downcast_ref::<DenoError>() {
            Some(DenoError::Explicit(error)) => Some(error.to_string()),
            _ => match root_error.downcast_ref::<SubsystemResolutionError>() {
                Some(error) => Some(error.user_error_message()),
                _ => match root_error.downcast_ref::<SystemResolutionError>() {
                    Some(error) => Some(error.user_error_message()),
                    _ => None,
                },
            },
        }
    }
}
