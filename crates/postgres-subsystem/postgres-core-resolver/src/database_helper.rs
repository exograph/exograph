use exo_env::Environment;
use exo_sql::{DatabaseClientManager, DatabaseExecutor, TransactionMode};
use thiserror::Error;
use tokio_postgres::{Row, types::FromSqlOwned};

use crate::postgres_execution_error::PostgresExecutionError;

pub async fn create_database_executor(
    existing_client: Option<DatabaseClientManager>,
    env: &dyn Environment,
) -> Result<DatabaseExecutor, DatabaseHelperError> {
    let database_client = if let Some(existing) = existing_client {
        existing
    } else {
        #[cfg(feature = "network")]
        {
            use common::env_const::{
                DATABASE_URL, EXO_CHECK_CONNECTION_ON_STARTUP, EXO_CONNECTION_POOL_SIZE,
                EXO_POSTGRES_READ_WRITE, EXO_POSTGRES_URL,
            };

            let url = env
                .get(EXO_POSTGRES_URL)
                .or(env.get(DATABASE_URL))
                .ok_or_else(|| {
                    DatabaseHelperError::Config("Env EXO_POSTGRES_URL not set".to_string())
                })?;
            let pool_size: Option<usize> = env
                .get(EXO_CONNECTION_POOL_SIZE)
                .and_then(|s| s.parse().ok());
            let check_connection = env
                .enabled(EXO_CHECK_CONNECTION_ON_STARTUP, true)
                .map_err(|e| DatabaseHelperError::BoxedError(Box::new(e)))?;
            let transaction_mode = if env
                .enabled(EXO_POSTGRES_READ_WRITE, false)
                .map_err(|e| DatabaseHelperError::BoxedError(Box::new(e)))?
            {
                TransactionMode::ReadWrite
            } else {
                TransactionMode::ReadOnly
            };

            DatabaseClientManager::from_url(&url, check_connection, pool_size, transaction_mode)
                .await
                .map_err(|e| DatabaseHelperError::BoxedError(Box::new(e)))?
        }

        #[cfg(not(feature = "network"))]
        {
            let _ = env;
            panic!("Postgres URL feature is not enabled");
        }
    };
    Ok(DatabaseExecutor { database_client })
}

#[derive(Error, Debug)]
pub enum DatabaseHelperError {
    #[error("Config error: {0}")]
    Config(String),

    #[error("Boxed error: {0}")]
    BoxedError(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}

pub fn extractor<T: FromSqlOwned>(row: Row) -> Result<T, PostgresExecutionError> {
    match row.try_get(0) {
        Ok(col) => Ok(col),
        Err(err) => Err(PostgresExecutionError::EmptyRow(err)),
    }
}
