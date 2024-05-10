// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::cell::RefCell;

use deadpool_postgres::Connect;
use tokio_postgres::Config;

use crate::database_error::DatabaseError;

use super::{database_client::DatabaseClient, database_pool::DatabasePool};

// we spawn many resolvers concurrently in integration tests
thread_local! {
    pub static LOCAL_URL: RefCell<Option<String>> = const { RefCell::new(None) };
    pub static LOCAL_CONNECTION_POOL_SIZE: RefCell<Option<usize>> = const { RefCell::new(None) };
    pub static LOCAL_CHECK_CONNECTION_ON_STARTUP: RefCell<Option<bool>> = const { RefCell::new(None) };
}

pub enum DatabaseClientManager {
    Pooled(DatabasePool),
}

impl DatabaseClientManager {
    #[cfg(feature = "postgres-url")]
    pub async fn from_env(pool_size_override: Option<usize>) -> Result<Self, DatabaseError> {
        let pool = DatabasePool::from_env(pool_size_override).await?;
        Ok(Self::Pooled(pool))
    }

    #[cfg(feature = "postgres-url")]
    pub async fn from_db_url(url: &str, check_connection: bool) -> Result<Self, DatabaseError> {
        let pool = DatabasePool::from_db_url(url, check_connection).await?;
        Ok(Self::Pooled(pool))
    }

    pub async fn from_connect(
        pool_size: usize,
        check_connection: bool,
        config: Config,
        connect: impl Connect + 'static,
        user: Option<String>,
        password: Option<String>,
    ) -> Result<Self, DatabaseError> {
        let pool = DatabasePool::from_connect(
            pool_size,
            check_connection,
            config,
            connect,
            user,
            password,
        )
        .await?;
        Ok(Self::Pooled(pool))
    }

    pub async fn get_client(&self) -> Result<DatabaseClient, DatabaseError> {
        match self {
            DatabaseClientManager::Pooled(pool) => pool.get_client().await,
        }
    }
}
