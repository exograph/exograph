use std::ops::{Deref, DerefMut};

use tokio_postgres::ToStatement;

pub enum DatabaseClient {
    Pooled(deadpool_postgres::Client),
    Raw(tokio_postgres::Client),
}

pub enum TransactionWrapper<'a> {
    Pooled(deadpool_postgres::Transaction<'a>),
    Raw(tokio_postgres::Transaction<'a>),
}

impl<'a> TransactionWrapper<'a> {
    pub async fn commit(self) -> Result<(), tokio_postgres::Error> {
        match self {
            TransactionWrapper::Pooled(tx) => tx.commit().await,
            TransactionWrapper::Raw(tx) => tx.commit().await,
        }
    }

    pub async fn rollback(self) -> Result<(), tokio_postgres::Error> {
        match self {
            TransactionWrapper::Pooled(tx) => tx.rollback().await,
            TransactionWrapper::Raw(tx) => tx.rollback().await,
        }
    }
}

impl<'a> Deref for TransactionWrapper<'a> {
    type Target = tokio_postgres::Transaction<'a>;

    fn deref(&self) -> &Self::Target {
        match self {
            TransactionWrapper::Pooled(tx) => tx,
            TransactionWrapper::Raw(tx) => tx,
        }
    }
}

impl<'a> DerefMut for TransactionWrapper<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            TransactionWrapper::Pooled(tx) => tx,
            TransactionWrapper::Raw(tx) => tx,
        }
    }
}

impl DatabaseClient {
    pub async fn transaction(
        &mut self,
    ) -> Result<TransactionWrapper<'_>, tokio_postgres::error::Error> {
        match self {
            DatabaseClient::Pooled(client) => {
                client.transaction().await.map(TransactionWrapper::Pooled)
            }
            DatabaseClient::Raw(client) => client.transaction().await.map(TransactionWrapper::Raw),
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
            DatabaseClient::Pooled(client) => client.query(query, params).await,
            DatabaseClient::Raw(client) => client.query(query, params).await,
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
            DatabaseClient::Pooled(client) => client.execute(query, params).await,
            DatabaseClient::Raw(client) => client.execute(query, params).await,
        }
    }
}
