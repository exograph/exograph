// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{
    ops::DerefMut,
    sync::{LazyLock, atomic::AtomicBool},
};

use tokio::sync::Mutex;

use exo_sql_core::{Database, database_error::DatabaseError};
use exo_sql_pg::transaction::{TransactionScript, TransactionStepResult};

use crate::{
    connect::{database_client::DatabaseClient, database_client_manager::DatabaseClientManager},
    execution::execute_transaction_script,
};

/// Manages the state of a database transaction across multiple operations within a request.
///
/// # Design: Manual BEGIN/COMMIT/ROLLBACK
///
/// Transactions are managed by sending explicit `BEGIN`, `COMMIT`, and `ROLLBACK` SQL commands
/// on the `DatabaseClient` rather than using `tokio_postgres::Transaction<'a>`.
///
/// The `Transaction<'a>` type borrows `&'a mut Client`, creating a self-referential struct when
/// both the client and transaction are stored together (as required). Rust's type system
/// cannot express this safely, so the previous implementation used `unsafe { std::mem::transmute }`
/// to erase the lifetime to `'static`. That approach had a critical flaw: forgetting to call
/// `finalize()` caused a use-after-free (the client was dropped before the transaction, whose
/// `Drop` impl tried to send ROLLBACK on the freed connection).
///
/// With manual `BEGIN`/`COMMIT`/`ROLLBACK`, there is no `Transaction` object borrowing from the
/// client, so no self-referential struct, no unsafe code, and forgetting `finalize()` simply
/// results in PostgreSQL's automatic rollback when the connection closes, which is a safe behavior.
///
/// # Lazy transaction creation
///
/// The transaction is deferred until the first database operation that requires one. This avoids
/// unnecessary `BEGIN`/`COMMIT` round-trips for requests that don't touch the database or that
/// only perform a single auto-committed query.
pub struct TransactionHolder {
    state: LazyLock<Mutex<TransactionState>>,
    needs_transaction: AtomicBool,
}

struct TransactionState {
    client: Option<DatabaseClient>,
    status: TransactionStatus,
}

#[derive(Debug, PartialEq)]
enum TransactionStatus {
    /// No transaction started (queries run in autocommit mode)
    Idle,
    /// BEGIN has been sent; queries run within the transaction
    Active,
    /// COMMIT or ROLLBACK has been sent
    Finalized,
}

impl Default for TransactionHolder {
    fn default() -> Self {
        Self {
            state: LazyLock::new(|| Mutex::new(TransactionState::new())),
            needs_transaction: AtomicBool::new(false),
        }
    }
}

impl TransactionHolder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the flag to indicate that a transaction must be used when executing work.
    ///
    /// Typically, a caller higher-up in the stack calls this method when it determines that a transaction
    /// must be used. (For example, when an operation has an interceptor and caller can't know if the interceptor
    /// will also do some database work).
    pub fn ensure_transaction(&self) {
        self.needs_transaction
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Execute work within a transaction context
    pub async fn with_tx(
        &mut self,
        database: &Database,
        client_manager: &DatabaseClientManager,
        work: TransactionScript<'_>,
    ) -> Result<TransactionStepResult, DatabaseError> {
        let mut state = self.state.lock().await;

        if state.status == TransactionStatus::Finalized {
            return Err(DatabaseError::Transaction(
                "Transaction already finalized".to_string(),
            ));
        }

        // Ensure we have a client
        state.ensure_client(client_manager).await?;

        // Execute the work
        let needs_tx = self
            .needs_transaction
            .load(std::sync::atomic::Ordering::SeqCst);
        state.execute_work(database, work, needs_tx).await
    }

    /// Finalize the transaction (commit or rollback based on parameter)
    pub async fn finalize(&mut self, commit: bool) -> Result<(), tokio_postgres::Error> {
        let mut state = self.state.lock().await;
        if commit {
            state.commit().await
        } else {
            state.rollback().await
        }
    }
}

impl TransactionState {
    fn new() -> Self {
        Self {
            client: None,
            status: TransactionStatus::Idle,
        }
    }

    async fn ensure_client(
        &mut self,
        client_manager: &DatabaseClientManager,
    ) -> Result<(), DatabaseError> {
        if self.client.is_none() {
            self.client = Some(client_manager.get_client().await?);
        }
        Ok(())
    }

    async fn begin_transaction(&mut self) -> Result<(), DatabaseError> {
        match self.status {
            TransactionStatus::Finalized => Err(DatabaseError::Transaction(
                "Transaction already finalized".to_string(),
            )),
            TransactionStatus::Active => Ok(()),
            TransactionStatus::Idle => match self.client {
                Some(ref client) => {
                    client
                        .batch_execute("BEGIN")
                        .await
                        .map_err(DatabaseError::driver)?;
                    self.status = TransactionStatus::Active;
                    Ok(())
                }
                None => Err(DatabaseError::Transaction(
                    "No database client available".to_string(),
                )),
            },
        }
    }

    async fn execute_work(
        &mut self,
        database: &Database,
        work: TransactionScript<'_>,
        needs_tx: bool,
    ) -> Result<TransactionStepResult, DatabaseError> {
        if work.needs_transaction() || needs_tx {
            self.begin_transaction().await?;
        }
        if let Some(ref mut client) = self.client {
            execute_transaction_script(work, database, client.deref_mut()).await
        } else {
            Err(DatabaseError::Transaction(
                "No database client available".to_string(),
            ))
        }
    }

    async fn commit(&mut self) -> Result<(), tokio_postgres::Error> {
        if self.status == TransactionStatus::Active
            && let Some(ref client) = self.client
        {
            client.batch_execute("COMMIT").await?;
        }
        self.status = TransactionStatus::Finalized;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), tokio_postgres::Error> {
        if self.status == TransactionStatus::Active
            && let Some(ref client) = self.client
        {
            client.batch_execute("ROLLBACK").await?;
        }
        self.status = TransactionStatus::Finalized;
        Ok(())
    }
}

impl Drop for TransactionState {
    fn drop(&mut self) {
        if self.status == TransactionStatus::Active {
            tracing::warn!(
                "TransactionState dropped with an open transaction that was not finalized. \
                 The server will implicitly roll back the transaction when the connection closes."
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_transaction_holder_creation() {
        let holder = TransactionHolder::new();
        assert_eq!(holder.state.lock().await.status, TransactionStatus::Idle);
    }

    #[test]
    fn test_ensure_transaction() {
        let holder = TransactionHolder::new();
        holder.ensure_transaction();
        assert!(
            holder
                .needs_transaction
                .load(std::sync::atomic::Ordering::SeqCst)
        );
    }

    #[tokio::test]
    async fn test_finalized_state_consistency() {
        let mut holder = TransactionHolder::new();

        // Initially not finalized
        assert_eq!(holder.state.lock().await.status, TransactionStatus::Idle);

        // After finalize, should be finalized (even without actual DB operations)
        let _ = holder.finalize(true).await;
        assert_eq!(
            holder.state.lock().await.status,
            TransactionStatus::Finalized
        );
    }
}

/// Tests that exercise transaction behavior against a real database.
///
/// These tests verify the manual BEGIN/COMMIT/ROLLBACK approach handles
/// various failure modes correctly (forgotten finalize, double finalize,
/// error recovery, etc.)
#[cfg(all(test, feature = "test-support"))]
mod database_tests {
    use crate::TransactionMode;
    use crate::connect::database_client_manager::DatabaseClientManager;
    use crate::testing::test_support;

    use super::*;

    const INIT_SCHEMA: &str = "CREATE TABLE test_items (id SERIAL PRIMARY KEY, name TEXT NOT NULL)";

    async fn client_manager(url: &str) -> DatabaseClientManager {
        DatabaseClientManager::from_url_direct(url, false, TransactionMode::ReadWrite)
            .await
            .unwrap()
    }

    /// Test helper to count rows in test_items using a fresh connection
    async fn count_items(url: &str) -> i64 {
        let mgr = client_manager(url).await;
        let client = mgr.get_client().await.unwrap();
        let row = client
            .query("SELECT COUNT(*)::bigint FROM test_items", &[])
            .await
            .unwrap();
        row[0].get::<_, i64>(0)
    }

    /// Test helper: create the schema and return a client manager for subsequent use
    async fn setup_schema(url: &str) -> DatabaseClientManager {
        let mgr = client_manager(url).await;
        let client = mgr.get_client().await.unwrap();
        client.batch_execute(INIT_SCHEMA).await.unwrap();
        drop(client);
        mgr
    }

    #[tokio::test]
    async fn test_commit_persists_data() {
        test_support::with_db_url(|url| async move {
            let mgr = setup_schema(&url).await;

            // Use TransactionHolder to insert within a transaction
            let mut holder = TransactionHolder::new();
            holder.ensure_transaction();

            {
                let mut state = holder.state.lock().await;
                state.ensure_client(&mgr).await.unwrap();
                state.begin_transaction().await.unwrap();
                let client = state.client.as_ref().unwrap();
                client
                    .execute("INSERT INTO test_items (name) VALUES ($1)", &[&"committed"])
                    .await
                    .unwrap();
            }

            holder.finalize(true).await.unwrap();

            assert_eq!(count_items(&url).await, 1);
        })
        .await;
    }

    #[tokio::test]
    async fn test_rollback_discards_data() {
        test_support::with_db_url(|url| async move {
            let mgr = setup_schema(&url).await;

            let mut holder = TransactionHolder::new();
            holder.ensure_transaction();

            {
                let mut state = holder.state.lock().await;
                state.ensure_client(&mgr).await.unwrap();
                state.begin_transaction().await.unwrap();
                let client = state.client.as_ref().unwrap();
                client
                    .execute(
                        "INSERT INTO test_items (name) VALUES ($1)",
                        &[&"rolled_back"],
                    )
                    .await
                    .unwrap();
            }

            holder.finalize(false).await.unwrap();

            assert_eq!(count_items(&url).await, 0);
        })
        .await;
    }

    /// Dropping TransactionHolder without calling finalize auto-rolls back
    /// (via PostgreSQL's implicit rollback on connection close).
    #[tokio::test]
    async fn test_drop_without_finalize_rolls_back() {
        test_support::with_db_url(|url| async move {
            let mgr = setup_schema(&url).await;

            // Scope the holder so it drops without finalize
            {
                let holder = TransactionHolder::new();
                let mut state = holder.state.lock().await;
                state.ensure_client(&mgr).await.unwrap();
                state.begin_transaction().await.unwrap();
                let client = state.client.as_ref().unwrap();
                client
                    .execute("INSERT INTO test_items (name) VALUES ($1)", &[&"orphaned"])
                    .await
                    .unwrap();
                // holder drops here without finalize -> implicit rollback
            }

            assert_eq!(count_items(&url).await, 0);
        })
        .await;
    }

    /// Calling finalize twice is safe (second call is a no-op).
    #[tokio::test]
    async fn test_double_finalize_is_safe() {
        test_support::with_db_url(|url| async move {
            let mgr = setup_schema(&url).await;

            let mut holder = TransactionHolder::new();
            holder.ensure_transaction();

            {
                let mut state = holder.state.lock().await;
                state.ensure_client(&mgr).await.unwrap();
                state.begin_transaction().await.unwrap();
                let client = state.client.as_ref().unwrap();
                client
                    .execute("INSERT INTO test_items (name) VALUES ($1)", &[&"double"])
                    .await
                    .unwrap();
            }

            holder.finalize(true).await.unwrap();
            // Second finalize should not error
            holder.finalize(true).await.unwrap();

            assert_eq!(count_items(&url).await, 1);
        })
        .await;
    }

    /// After a query error inside a transaction, ROLLBACK recovers the connection.
    #[tokio::test]
    async fn test_error_in_transaction_then_rollback() {
        test_support::with_db_url(|url| async move {
            let mgr = setup_schema(&url).await;

            let mut holder = TransactionHolder::new();
            holder.ensure_transaction();

            {
                let mut state = holder.state.lock().await;
                state.ensure_client(&mgr).await.unwrap();
                state.begin_transaction().await.unwrap();
                let client = state.client.as_ref().unwrap();

                // This INSERT succeeds
                client
                    .execute(
                        "INSERT INTO test_items (name) VALUES ($1)",
                        &[&"before_error"],
                    )
                    .await
                    .unwrap();

                // This query fails (invalid SQL referencing nonexistent table)
                let err = client
                    .execute("INSERT INTO nonexistent_table VALUES ($1)", &[&"bad"])
                    .await;
                assert!(err.is_err());

                // Transaction is now in aborted state — further queries would fail
            }

            // Rollback should succeed and recover the connection
            holder.finalize(false).await.unwrap();

            // Nothing was committed
            assert_eq!(count_items(&url).await, 0);
        })
        .await;
    }

    /// Without ensure_transaction, operations run without a transaction (autocommit).
    #[tokio::test]
    async fn test_autocommits() {
        test_support::with_db_url(|url| async move {
            let mgr = setup_schema(&url).await;

            let holder = TransactionHolder::new();
            // Note: NOT calling holder.ensure_transaction()

            {
                let mut state = holder.state.lock().await;
                state.ensure_client(&mgr).await.unwrap();
                // execute_work with needs_tx=false should NOT begin a transaction
                assert_eq!(state.status, TransactionStatus::Idle);
                let client = state.client.as_ref().unwrap();
                client
                    .execute(
                        "INSERT INTO test_items (name) VALUES ($1)",
                        &[&"autocommit"],
                    )
                    .await
                    .unwrap();
            }

            // Even without finalize, data persists (autocommit mode)
            assert_eq!(count_items(&url).await, 1);
        })
        .await;
    }

    /// with_tx after finalize returns an error.
    #[tokio::test]
    async fn test_with_tx_after_finalize_errors() {
        test_support::with_db_url(|url| async move {
            let _mgr = setup_schema(&url).await;

            let mut holder = TransactionHolder::new();

            // Finalize without any work
            holder.finalize(true).await.unwrap();

            // Now try to begin a transaction — should fail
            let mut state = holder.state.lock().await;
            let result = state.begin_transaction().await;
            assert!(matches!(result, Err(DatabaseError::Transaction(_))));
        })
        .await;
    }
}
