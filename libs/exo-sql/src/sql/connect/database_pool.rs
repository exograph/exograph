#![cfg(feature = "pool")]

#[cfg(feature = "postgres-url")]
use std::env;

#[cfg(feature = "postgres-url")]
use common::env_const::{
    EXO_CONNECTION_POOL_SIZE, EXO_POSTGRES_PASSWORD, EXO_POSTGRES_URL, EXO_POSTGRES_USER,
};

#[cfg(feature = "postgres-url")]
use deadpool_postgres::ConfigConnectImpl;
use deadpool_postgres::{Connect, Manager, ManagerConfig, Pool, RecyclingMethod};

use tokio_postgres::Config;

use crate::database_error::DatabaseError;

use super::{creation::DatabaseCreation, database_client::DatabaseClient};

pub struct DatabasePool {
    pool: Pool,
}

impl DatabasePool {
    pub async fn create(
        creation: DatabaseCreation,
        pool_size: Option<usize>,
    ) -> Result<Self, DatabaseError> {
        match creation {
            DatabaseCreation::Env => Self::from_env(pool_size).await,
            DatabaseCreation::Url { url } => Self::from_db_url(&url, pool_size).await,
            DatabaseCreation::Connect {
                config,
                user,
                password,
                connect,
            } => Self::from_connect(1, *config, ConnectBridge(connect), user, password).await,
        }
    }

    pub async fn get_client(&self) -> Result<DatabaseClient, DatabaseError> {
        Ok(DatabaseClient::Pooled(self.pool.get().await?))
    }

    // pool_size_override useful when we want to explicitly control the pool size (for example, to 1, when importing database schema)
    #[cfg(feature = "postgres-url")]
    async fn from_env(pool_size_override: Option<usize>) -> Result<Self, DatabaseError> {
        use crate::{LOCAL_CONNECTION_POOL_SIZE, LOCAL_URL};

        let url = LOCAL_URL
            .with(|f| f.borrow().clone())
            .or_else(|| env::var(EXO_POSTGRES_URL).ok())
            .ok_or(DatabaseError::Config(format!(
                "Env {EXO_POSTGRES_URL} must be provided"
            )))?;

        let user = env::var(EXO_POSTGRES_USER).ok();
        let password = env::var(EXO_POSTGRES_PASSWORD).ok();
        let pool_size = pool_size_override.unwrap_or_else(|| {
            LOCAL_CONNECTION_POOL_SIZE
                .with(|f| *f.borrow())
                .or_else(|| {
                    env::var(EXO_CONNECTION_POOL_SIZE)
                        .ok()
                        .map(|pool_str| pool_str.parse::<usize>().unwrap())
                })
                .unwrap_or(10)
        });

        Self::from_helper(pool_size, &url, user, password).await
    }

    #[cfg(feature = "postgres-url")]
    async fn from_db_url(url: &str, pool_size: Option<usize>) -> Result<Self, DatabaseError> {
        Self::from_helper(pool_size.unwrap_or(1), url, None, None).await
    }

    #[cfg(feature = "postgres-url")]
    async fn from_helper(
        pool_size: usize,
        url: &str,
        user: Option<String>,
        password: Option<String>,
    ) -> Result<Self, DatabaseError> {
        use std::str::FromStr;

        use crate::sql::connect::ssl_config::SslConfig;

        let (url, ssl_config) = SslConfig::from_url(url)?;

        let config = Config::from_str(&url).map_err(|e| {
            DatabaseError::Delegate(e)
                .with_context("Failed to parse PostgreSQL connection string".into())
        })?;

        match ssl_config {
            Some(ssl_config) => {
                let (config, tls) = ssl_config.updated_config(config)?;

                Self::from_connect(pool_size, config, ConfigConnectImpl { tls }, user, password)
                    .await
            }
            None => {
                let tls = tokio_postgres::NoTls;
                Self::from_connect(pool_size, config, ConfigConnectImpl { tls }, user, password)
                    .await
            }
        }
    }

    pub async fn from_connect(
        pool_size: usize,
        mut config: Config,
        connect: impl Connect + 'static,
        user: Option<String>,
        password: Option<String>,
    ) -> Result<Self, DatabaseError> {
        if let Some(user) = &user {
            config.user(user);
        }
        if let Some(password) = &password {
            config.password(password);
        }

        let manager_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        };

        let manager = { Manager::from_connect(config, connect, manager_config) };

        let pool = Pool::builder(manager)
            .max_size(pool_size)
            .build()
            .expect("Failed to create DB pool");

        let db = Self { pool };

        Ok(db)
    }
}

struct ConnectBridge(Box<dyn super::creation::Connect>);

impl Connect for ConnectBridge {
    fn connect(
        &self,
        pg_config: &tokio_postgres::Config,
    ) -> futures::future::BoxFuture<
        '_,
        Result<(tokio_postgres::Client, tokio::task::JoinHandle<()>), tokio_postgres::Error>,
    > {
        self.0.connect(pg_config)
    }
}
