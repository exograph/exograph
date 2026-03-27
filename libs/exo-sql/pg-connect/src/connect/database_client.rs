// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::ops::{Deref, DerefMut};

use tokio_postgres::ToStatement;

pub enum DatabaseClient {
    #[cfg(feature = "pool")]
    Pooled(deadpool_postgres::Client),
    Direct(tokio_postgres::Client),
}

impl Deref for DatabaseClient {
    type Target = tokio_postgres::Client;

    fn deref(&self) -> &Self::Target {
        match self {
            #[cfg(feature = "pool")]
            DatabaseClient::Pooled(client) => client,
            DatabaseClient::Direct(client) => client,
        }
    }
}

impl DerefMut for DatabaseClient {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            #[cfg(feature = "pool")]
            DatabaseClient::Pooled(client) => client,
            DatabaseClient::Direct(client) => client,
        }
    }
}

impl DatabaseClient {
    pub async fn query<T>(
        &self,
        query: &T,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> Result<Vec<tokio_postgres::Row>, tokio_postgres::Error>
    where
        T: ?Sized + ToStatement,
    {
        match self {
            #[cfg(feature = "pool")]
            DatabaseClient::Pooled(client) => client.query(query, params).await,
            DatabaseClient::Direct(client) => client.query(query, params).await,
        }
    }

    pub async fn execute<T>(
        &self,
        query: &T,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> Result<u64, tokio_postgres::Error>
    where
        T: ?Sized + ToStatement,
    {
        match self {
            #[cfg(feature = "pool")]
            DatabaseClient::Pooled(client) => client.execute(query, params).await,
            DatabaseClient::Direct(client) => client.execute(query, params).await,
        }
    }
}
