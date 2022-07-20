use std::error::Error;

use base64::DecodeError;
use payas_deno::deno_error::DenoError;
use thiserror::Error;

use crate::validation_error::ValidationError;

#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("{0}")]
    Generic(String),

    #[error(transparent)]
    Database(#[from] DatabaseExecutionError),

    #[error(transparent)]
    Service(#[from] ServiceExecutionError),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error(transparent)]
    Validation(#[from] ValidationError),

    #[error(transparent)]
    CastError(#[from] CastError),

    #[error("Invalid field {0} for {1}")]
    InvalidField(String, &'static str), // (field name, container type)

    #[error("Not authorized")]
    Authorization,

    #[error("{0} {1}")]
    WithContext(String, #[source] Box<ExecutionError>),
}

#[derive(Error, Debug)]
pub enum DatabaseExecutionError {
    #[error(transparent)]
    Database(#[from] payas_sql::database_error::DatabaseError),

    #[error(transparent)]
    EmptyRow(#[from] tokio_postgres::Error),

    #[error("Result has {0} entries; expected only zero or one")]
    NonUniqueResult(usize),
}

#[derive(Error, Debug)]
pub enum ServiceExecutionError {
    #[error(transparent)]
    Deno(#[from] DenoError),

    #[error("Invalid argument {0}")]
    InvalidArgument(String),
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

#[derive(Debug, Error)]
pub enum CastError {
    #[error("{0}")]
    Generic(String),

    #[error("{0}")]
    Date(String, #[source] chrono::format::ParseError),

    #[error(transparent)]
    Blob(#[from] DecodeError),

    #[error(transparent)]
    Uuid(#[from] uuid::Error),

    #[error("{0}")]
    BigDecimal(String),

    #[error(transparent)]
    Database(#[from] payas_sql::database_error::DatabaseError),
}
