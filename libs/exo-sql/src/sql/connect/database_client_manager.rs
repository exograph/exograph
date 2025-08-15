// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{Connect, database_error::DatabaseError};

use super::{
    creation::{DatabaseCreation, TransactionMode},
    database_client::DatabaseClient,
};

#[cfg(feature = "pool")]
use super::database_pool::DatabasePool;

pub enum DatabaseClientManager {
    #[cfg(feature = "pool")]
    Pooled(DatabasePool),
    Direct(DatabaseCreation),
}

impl DatabaseClientManager {
    pub async fn from_connect_direct(
        check_connection: bool,
        config: tokio_postgres::Config,
        connect: impl Connect + 'static,
    ) -> Result<Self, DatabaseError> {
        let creation = DatabaseCreation::Connect {
            config: Box::new(config),
            connect: Box::new(connect),
        };

        let res = Ok(Self::Direct(creation));

        if let Ok(ref res) = res
            && check_connection
        {
            let _ = res.get_client().await?;
        }

        res
    }

    #[cfg(feature = "pool")]
    pub async fn from_connect_pooled(
        check_connection: bool,
        config: tokio_postgres::Config,
        connect: impl Connect + 'static,
        pool_size: usize,
    ) -> Result<Self, DatabaseError> {
        let creation = DatabaseCreation::Connect {
            config: Box::new(config),
            connect: Box::new(connect),
        };

        let res = Ok(Self::Pooled(
            DatabasePool::create(creation, Some(pool_size)).await?,
        ));

        if let Ok(ref res) = res
            && check_connection
        {
            let _ = res.get_client().await?;
        }

        res
    }

    pub async fn get_client(&self) -> Result<DatabaseClient, DatabaseError> {
        match self {
            #[cfg(feature = "pool")]
            DatabaseClientManager::Pooled(pool) => pool.get_client().await,
            DatabaseClientManager::Direct(creation) => creation.get_client().await,
        }
    }
}

#[cfg(feature = "postgres-url")]
impl DatabaseClientManager {
    pub async fn from_url(
        url: &str,
        check_connection: bool,
        #[allow(unused_variables)] pool_size: Option<usize>,
        transaction_mode: TransactionMode,
    ) -> Result<Self, DatabaseError> {
        #[cfg(feature = "pool")]
        {
            Self::from_url_pooled(url, check_connection, pool_size, transaction_mode).await
        }
        #[cfg(not(feature = "pool"))]
        {
            Self::from_url_direct(url, check_connection, transaction_mode).await
        }
    }

    pub async fn from_url_direct(
        url: &str,
        check_connection: bool,
        transaction_mode: TransactionMode,
    ) -> Result<Self, DatabaseError> {
        let creation = DatabaseCreation::Url {
            url: url.to_string(),
            transaction_mode,
        };
        let res = Ok(DatabaseClientManager::Direct(creation));

        if let Ok(ref res) = res
            && check_connection
        {
            let _ = res.get_client().await?;
        }

        res
    }

    #[cfg(feature = "pool")]
    pub async fn from_url_pooled(
        url: &str,
        check_connection: bool,
        pool_size: Option<usize>,
        transaction_mode: TransactionMode,
    ) -> Result<Self, DatabaseError> {
        let creation = DatabaseCreation::Url {
            url: url.to_string(),
            transaction_mode,
        };
        let res = Ok(Self::Pooled(
            DatabasePool::create(creation, pool_size).await?,
        ));

        if let Ok(ref res) = res
            && check_connection
        {
            let _ = res.get_client().await?;
        }

        res
    }
}
