use std::env;

use common::env_const::{EXO_POSTGRES_PASSWORD, EXO_POSTGRES_URL, EXO_POSTGRES_USER};
use futures::future::BoxFuture;
use tokio::task::JoinHandle;
use tokio_postgres::Config;

use crate::database_error::DatabaseError;

use super::database_client::DatabaseClient;

pub enum DatabaseCreation {
    Env,
    Url {
        url: String,
    },
    Connect {
        config: Box<Config>,
        user: Option<String>,
        password: Option<String>,
        connect: Box<dyn Connect>,
    },
}

impl DatabaseCreation {
    pub async fn get_client(&self) -> Result<DatabaseClient, DatabaseError> {
        match self {
            DatabaseCreation::Connect {
                config, connect, ..
            } => {
                let fut = connect.connect(config);
                let (client, connection) = fut.await?;
                let _conn_task = tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        tracing::error!(target: "Postgres", "Connection error: {}", e);
                    }
                });
                Ok(DatabaseClient::Direct(client))
            }
            DatabaseCreation::Env => {
                use crate::LOCAL_URL;

                let url = LOCAL_URL
                    .with(|f| f.borrow().clone())
                    .or_else(|| env::var(EXO_POSTGRES_URL).ok())
                    .ok_or(DatabaseError::Config(format!(
                        "Env {EXO_POSTGRES_URL} must be provided"
                    )))?;

                let user = env::var(EXO_POSTGRES_USER).ok();
                let password = env::var(EXO_POSTGRES_PASSWORD).ok();

                Self::from_helper(&url, user, password).await
            }
            DatabaseCreation::Url { url } => Self::from_helper(url, None, None).await,
        }
    }

    async fn from_helper(
        url: &str,
        user: Option<String>,
        password: Option<String>,
    ) -> Result<DatabaseClient, DatabaseError> {
        use std::str::FromStr;

        use crate::sql::connect::ssl_config::SslConfig;

        let (url, ssl_config) = SslConfig::from_url(url)?;

        let mut config = Config::from_str(&url).map_err(|e| {
            DatabaseError::Delegate(e)
                .with_context("Failed to parse PostgreSQL connection string".into())
        })?;

        if let Some(user) = &user {
            config.user(user);
        }
        if let Some(password) = &password {
            config.password(password);
        }

        match ssl_config {
            Some(ssl_config) => {
                let (config, tls) = ssl_config.updated_config(config)?;

                let (client, connection) = config.connect(tls).await?;

                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        tracing::error!("connection error: {}", e);
                    }
                });

                Ok(DatabaseClient::Direct(client))
            }
            None => {
                let tls = tokio_postgres::NoTls;
                let (client, connection) = config.connect(tls).await?;

                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        tracing::error!("connection error: {}", e);
                    }
                });

                Ok(DatabaseClient::Direct(client))
            }
        }
    }
}

/// A trait for connecting to a database.
///
/// Implementation note: This is the same as deadpool_postgres::Connect, but allows to be used even
/// when the "pool" feature is not enabled.
pub trait Connect: Sync + Send {
    fn connect(
        &self,
        pg_config: &tokio_postgres::Config,
    ) -> BoxFuture<'_, Result<(tokio_postgres::Client, JoinHandle<()>), tokio_postgres::Error>>;
}
