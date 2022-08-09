use thiserror::Error;

use payas_deno::deno_error::DenoError;

#[derive(Error, Debug)]
pub enum DenoExecutionError {
    #[error(transparent)]
    Deno(#[from] DenoError),

    #[error("Invalid argument {0}")]
    InvalidArgument(String),

    #[error("Not authorized")]
    Authorization,

    #[error(transparent)]
    Delegate(#[from] Box<dyn std::error::Error + Send + Sync>),
}

impl DenoExecutionError {
    pub fn user_error_message(&self) -> String {
        match self {
            DenoExecutionError::Authorization => "Not authorized".to_string(),
            DenoExecutionError::Deno(DenoError::Explicit(error)) => error.to_string(),
            _ => "Internal server error".to_string(), // Do not reveal too much information about the error
        }
    }
}
