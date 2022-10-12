use thiserror::Error;

use super::cast::CastError;

#[derive(Error, Debug)]
pub enum DatabaseExecutionError {
    #[error("{0}")]
    Generic(String),

    #[error("{0}")]
    Validation(String),

    #[error("{0}")]
    Database(#[from] payas_sql::database_error::DatabaseError),

    #[error("{0}")]
    EmptyRow(#[from] tokio_postgres::Error),

    #[error("Result has {0} entries; expected only zero or one")]
    NonUniqueResult(usize),

    #[error("{0}")]
    CastError(#[from] CastError),

    #[error("Not authorized")]
    Authorization,

    #[error("{0} {1}")]
    WithContext(String, #[source] Box<DatabaseExecutionError>),
}

impl DatabaseExecutionError {
    pub fn with_context(self, context: String) -> DatabaseExecutionError {
        DatabaseExecutionError::WithContext(context, Box::new(self))
    }

    pub fn user_error_message(&self) -> String {
        match self {
            DatabaseExecutionError::Authorization => "Not authorized".to_string(),
            DatabaseExecutionError::Validation(message) => message.to_string(),
            // Do not reveal the underlying database error as it may expose sensitive details (such as column names or data involved in constraint violation).
            _ => "Database operation failed".to_string(),
        }
    }
}
pub(crate) trait WithContext {
    fn with_context(self, context: String) -> Self;
}

impl<T> WithContext for Result<T, DatabaseExecutionError> {
    fn with_context(self, context: String) -> Result<T, DatabaseExecutionError> {
        self.map_err(|e| e.with_context(context))
    }
}
