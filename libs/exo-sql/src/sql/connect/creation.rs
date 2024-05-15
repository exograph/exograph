// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use futures::future::BoxFuture;
use tokio::task::JoinHandle;
use tokio_postgres::Config;

use crate::database_error::DatabaseError;

use super::database_client::DatabaseClient;

pub enum DatabaseCreation {
    #[cfg(feature = "postgres-url")]
    Url { url: String },
    Connect {
        config: Box<Config>,
        connect: Box<dyn Connect>,
    },
}

impl DatabaseCreation {
    pub async fn get_client(&self) -> Result<DatabaseClient, DatabaseError> {
        match self {
            DatabaseCreation::Connect {
                config, connect, ..
            } => {
                let (client, _connection) = connect.connect(config).await?;
                Ok(DatabaseClient::Direct(client))
            }
            #[cfg(feature = "postgres-url")]
            DatabaseCreation::Url { url } => Self::from_url(url).await,
        }
    }

    #[cfg(feature = "postgres-url")]
    async fn from_url(url: &str) -> Result<DatabaseClient, DatabaseError> {
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
