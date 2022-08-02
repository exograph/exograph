use thiserror::Error;

use super::cast::CastError;

#[derive(Error, Debug)]
pub enum DatabaseExecutionError {
    #[error("{0}")]
    Generic(String),

    #[error(transparent)]
    Database(#[from] payas_sql::database_error::DatabaseError),

    #[error(transparent)]
    EmptyRow(#[from] tokio_postgres::Error),

    #[error("Result has {0} entries; expected only zero or one")]
    NonUniqueResult(usize),

    #[error(transparent)]
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
}
pub trait WithContext {
    fn with_context(self, context: String) -> Self;
}

impl<T> WithContext for Result<T, DatabaseExecutionError> {
    fn with_context(self, context: String) -> Result<T, DatabaseExecutionError> {
        self.map_err(|e| e.with_context(context))
    }
}
