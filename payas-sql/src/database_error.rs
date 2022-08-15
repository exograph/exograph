use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Failed to execute transaction {0}")]
    Transaction(String),

    #[error("{0}")]
    Validation(String),

    #[error("{0}")]
    Delegate(#[from] tokio_postgres::Error),

    #[error("{0}")]
    Ssl(#[from] openssl::error::ErrorStack),

    #[error("{0}")]
    Pool(#[from] deadpool_postgres::PoolError),

    #[error("{0} {1}")]
    WithContext(String, #[source] Box<DatabaseError>),

    #[error("{0}")]
    BoxedError(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl DatabaseError {
    pub fn with_context(self, context: String) -> DatabaseError {
        DatabaseError::WithContext(context, Box::new(self))
    }
}

pub trait WithContext {
    fn with_context(self, context: String) -> Self;
}

impl<T> WithContext for Result<T, DatabaseError> {
    fn with_context(self, context: String) -> Result<T, DatabaseError> {
        self.map_err(|e| e.with_context(context))
    }
}
