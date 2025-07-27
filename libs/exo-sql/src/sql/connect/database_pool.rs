// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

#![cfg(feature = "pool")]

#[cfg(feature = "postgres-url")]
use deadpool_postgres::ConfigConnectImpl;
use deadpool_postgres::{Connect, Manager, ManagerConfig, Pool, RecyclingMethod};

use tokio_postgres::Config;

use crate::TransactionMode;
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
            #[cfg(feature = "postgres-url")]
            DatabaseCreation::Url {
                url,
                transaction_mode,
            } => Self::from_db_url(&url, pool_size, transaction_mode).await,
            DatabaseCreation::Connect { config, connect } => {
                Self::from_connect(pool_size, *config, ConnectBridge(connect)).await
            }
        }
    }

    pub async fn get_client(&self) -> Result<DatabaseClient, DatabaseError> {
        Ok(DatabaseClient::Pooled(self.pool.get().await?))
    }

    #[cfg(feature = "postgres-url")]
    async fn from_db_url(
        url: &str,
        pool_size: Option<usize>,
        transaction_mode: TransactionMode,
    ) -> Result<Self, DatabaseError> {
        Self::from_helper(url, pool_size, transaction_mode).await
    }

    #[cfg(feature = "postgres-url")]
    async fn from_helper(
        url: &str,
        pool_size: Option<usize>,
        transaction_mode: TransactionMode,
    ) -> Result<Self, DatabaseError> {
        use std::str::FromStr;

        use crate::sql::connect::ssl_config::SslConfig;

        let (url, ssl_config) = SslConfig::from_url(url)?;

        let mut config = Config::from_str(&url).map_err(|e| {
            DatabaseError::Delegate(e)
                .with_context("Failed to parse PostgreSQL connection string".into())
        })?;

        transaction_mode.update_config(&mut config);

        match ssl_config {
            Some(ssl_config) => {
                let (config, tls) = ssl_config.updated_config(config)?;

                // If there is any TCP host, use the TLS connector (with the new Rustls version, SSL over unix sockets errors out)
                let has_tcp_hosts = config
                    .get_hosts()
                    .iter()
                    .any(|host| matches!(host, tokio_postgres::config::Host::Tcp(_)));

                if has_tcp_hosts {
                    Self::from_connect(pool_size, config, ConfigConnectImpl { tls }).await
                } else {
                    Self::from_connect(
                        pool_size,
                        config,
                        ConfigConnectImpl {
                            tls: tokio_postgres::NoTls,
                        },
                    )
                    .await
                }
            }
            None => {
                Self::from_connect(
                    pool_size,
                    config,
                    ConfigConnectImpl {
                        tls: tokio_postgres::NoTls,
                    },
                )
                .await
            }
        }
    }

    pub async fn from_connect(
        pool_size: Option<usize>,
        config: Config,
        connect: impl Connect + 'static,
    ) -> Result<Self, DatabaseError> {
        let manager_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        };

        let manager = Manager::from_connect(config, connect, manager_config);

        let pool = Pool::builder(manager);

        let pool = match pool_size {
            Some(pool_size) => pool.max_size(pool_size),
            None => pool,
        }
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
