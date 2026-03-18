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

/// Abstracts over the different transaction types that can be returned by the database client.
pub enum TransactionWrapper<'a> {
    #[cfg(feature = "pool")]
    Pooled(deadpool_postgres::Transaction<'a>),
    Direct(tokio_postgres::Transaction<'a>),
}

impl TransactionWrapper<'_> {
    pub async fn commit(self) -> Result<(), tokio_postgres::Error> {
        match self {
            #[cfg(feature = "pool")]
            TransactionWrapper::Pooled(tx) => tx.commit().await,
            TransactionWrapper::Direct(tx) => tx.commit().await,
        }
    }

    pub async fn rollback(self) -> Result<(), tokio_postgres::Error> {
        match self {
            #[cfg(feature = "pool")]
            TransactionWrapper::Pooled(tx) => tx.rollback().await,
            TransactionWrapper::Direct(tx) => tx.rollback().await,
        }
    }
}

impl<'a> Deref for TransactionWrapper<'a> {
    type Target = tokio_postgres::Transaction<'a>;

    fn deref(&self) -> &Self::Target {
        match self {
            #[cfg(feature = "pool")]
            TransactionWrapper::Pooled(tx) => tx,
            TransactionWrapper::Direct(tx) => tx,
        }
    }
}

impl DerefMut for TransactionWrapper<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            #[cfg(feature = "pool")]
            TransactionWrapper::Pooled(tx) => tx,
            TransactionWrapper::Direct(tx) => tx,
        }
    }
}

impl DatabaseClient {
    pub async fn transaction(
        &mut self,
    ) -> Result<TransactionWrapper<'_>, tokio_postgres::error::Error> {
        match self {
            #[cfg(feature = "pool")]
            DatabaseClient::Pooled(client) => {
                client.transaction().await.map(TransactionWrapper::Pooled)
            }
            DatabaseClient::Direct(client) => {
                client.transaction().await.map(TransactionWrapper::Direct)
            }
        }
    }

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
