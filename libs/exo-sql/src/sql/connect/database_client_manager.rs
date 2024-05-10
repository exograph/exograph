// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::cell::RefCell;

use crate::{database_error::DatabaseError, Connect};

use super::{creation::DatabaseCreation, database_client::DatabaseClient};

#[cfg(feature = "pool")]
use super::database_pool::DatabasePool;

// we spawn many resolvers concurrently in integration tests
thread_local! {
    pub static LOCAL_URL: RefCell<Option<String>> = const { RefCell::new(None) };
    pub static LOCAL_CONNECTION_POOL_SIZE: RefCell<Option<usize>> = const { RefCell::new(None) };
    pub static LOCAL_CHECK_CONNECTION_ON_STARTUP: RefCell<Option<bool>> = const { RefCell::new(None) };
}

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
        user: Option<String>,
        password: Option<String>,
    ) -> Result<Self, DatabaseError> {
        let creation = DatabaseCreation::Connect {
            config: Box::new(config),
            user,
            password,
            connect: Box::new(connect),
        };

        let res = Ok(Self::Direct(creation));

        if let Ok(ref res) = res {
            if check_connection {
                let _ = res.get_client().await?;
            }
        }

        res
    }

    #[cfg(feature = "pool")]
    pub async fn from_connect_pooled(
        check_connection: bool,
        config: tokio_postgres::Config,
        connect: impl Connect + 'static,
        user: Option<String>,
        password: Option<String>,
        pool_size: usize,
    ) -> Result<Self, DatabaseError> {
        let creation = DatabaseCreation::Connect {
            config: Box::new(config),
            user,
            password,
            connect: Box::new(connect),
        };

        let res = Ok(Self::Pooled(
            DatabasePool::create(creation, Some(pool_size)).await?,
        ));

        if let Ok(ref res) = res {
            if check_connection {
                let _ = res.get_client().await?;
            }
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
use std::env;

#[cfg(feature = "postgres-url")]
impl DatabaseClientManager {
    pub async fn from_env(pool_size_override: Option<usize>) -> Result<Self, DatabaseError> {
        #[cfg(feature = "pool")]
        {
            Self::from_env_pooled(pool_size_override).await
        }
        #[cfg(not(feature = "pool"))]
        {
            Self::from_env_direct().await
        }
    }

    pub async fn from_env_direct() -> Result<Self, DatabaseError> {
        let creation = DatabaseCreation::Env;
        Ok(DatabaseClientManager::Direct(creation))
    }

    #[cfg(feature = "pool")]
    pub async fn from_env_pooled(pool_size_override: Option<usize>) -> Result<Self, DatabaseError> {
        use common::env_const::EXO_CHECK_CONNECTION_ON_STARTUP;

        let creation = DatabaseCreation::Env;

        let res = Ok(Self::Pooled(
            DatabasePool::create(creation, pool_size_override).await?,
        ));

        let check_connection = LOCAL_CHECK_CONNECTION_ON_STARTUP
            .with(|f| *f.borrow())
            .or_else(|| {
                env::var(EXO_CHECK_CONNECTION_ON_STARTUP)
                    .ok()
                    .map(|check| check.parse::<bool>().expect("Should be true or false"))
            })
            .unwrap_or(true);

        if let Ok(ref res) = res {
            if check_connection {
                let _ = res.get_client().await?;
            }
        }

        res
    }

    pub async fn from_db_url(
        url: &str,
        check_connection: bool,
        pool_size: Option<usize>,
    ) -> Result<Self, DatabaseError> {
        #[cfg(feature = "pool")]
        {
            Self::from_db_url_pooled(url, check_connection, pool_size).await
        }
        #[cfg(not(feature = "pool"))]
        {
            Self::from_db_url_direct(url, check_connection).await
        }
    }

    pub async fn from_db_url_direct(
        url: &str,
        check_connection: bool,
    ) -> Result<Self, DatabaseError> {
        let creation = DatabaseCreation::Url {
            url: url.to_string(),
        };
        let res = Ok(DatabaseClientManager::Direct(creation));

        if let Ok(ref res) = res {
            if check_connection {
                let _ = res.get_client().await?;
            }
        }

        res
    }

    #[cfg(feature = "pool")]
    pub async fn from_db_url_pooled(
        url: &str,
        check_connection: bool,
        pool_size: Option<usize>,
    ) -> Result<Self, DatabaseError> {
        let creation = DatabaseCreation::Url {
            url: url.to_string(),
        };
        let res = Ok(Self::Pooled(
            DatabasePool::create(creation, pool_size).await?,
        ));

        if let Ok(ref res) = res {
            if check_connection {
                let _ = res.get_client().await?;
            }
        }

        res
    }
}
