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

use crate::{
    Database,
    database_error::DatabaseError,
    sql::{
        connect::{
            database_client::{DatabaseClient, TransactionWrapper},
            database_client_manager::DatabaseClientManager,
        },
        transaction::{TransactionScript, TransactionStepResult},
    },
};

/// Manages the state of a transaction.
///
/// The implementation complexity comes from the requirement that we must defer the creation of the transaction
/// until the first database operation is performed and must finalize (commit or rollback) after the last database operation.
/// For example, we may have a Deno operation that may execute multiple queries/mutations. We need to create a transaction
/// before the first database operation and finalize only when we are about to return results. Note that we can't
/// just wrap the work in a transaction because, for example, a Deno operation may not do any database work at all.
pub struct TransactionHolder {
    state: LazyLock<Mutex<TransactionState>>,
    needs_transaction: AtomicBool,
}

struct TransactionState {
    client: Option<DatabaseClient>,
    transaction: Option<TransactionWrapper<'static>>,
    finalized: bool,
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
    pub(super) async fn with_tx(
        &mut self,
        database: &Database,
        client_manager: &DatabaseClientManager,
        work: TransactionScript<'_>,
    ) -> Result<TransactionStepResult, DatabaseError> {
        let mut state = self.state.lock().await;

        if state.finalized {
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
            transaction: None,
            finalized: false,
        }
    }

    async fn ensure_client(
        &mut self,
        client_manager: &DatabaseClientManager,
    ) -> Result<(), DatabaseError> {
        if self.client.is_none() && !self.finalized {
            self.client = Some(client_manager.get_client().await?);
        }
        Ok(())
    }

    async fn ensure_transaction(
        &mut self,
    ) -> Result<&mut TransactionWrapper<'static>, DatabaseError> {
        if self.finalized {
            return Err(DatabaseError::Transaction(
                "Transaction already finalized".to_string(),
            ));
        }

        match self.transaction {
            Some(ref mut tx) => Ok(tx),
            None => match self.client {
                Some(ref mut client) => {
                    let tx = client.transaction().await?;

                    // SAFETY: This lifetime extension is safe because:
                    // 1. The TransactionWrapper<'_> borrows from the DatabaseClient (see DatabaseClient::transaction)
                    // 2. Both the client and transaction are stored in the same struct (TransactionState)
                    // 3. All fields of TransactionState and TransactionHolder are private, thus their access is only in this module
                    // 4. The transaction is only accessed through methods that ensure the client is still alive
                    // 5. Both are protected by the same Mutex<TransactionState> ensuring exclusive access
                    // 6. The transaction is always dropped before or with the client in commit/rollback
                    // 7. The 'static lifetime here is for the type system, but the actual lifetime
                    //    is managed by the containing struct which ensures memory safety
                    let tx_static: TransactionWrapper<'static> = unsafe { std::mem::transmute(tx) };

                    self.transaction = Some(tx_static);
                    Ok(self.transaction.as_mut().unwrap())
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
            let tx = self.ensure_transaction().await?;
            work.execute(database, tx.deref_mut()).await
        } else if let Some(ref mut client) = self.client {
            work.execute(database, client.deref_mut()).await
        } else {
            Err(DatabaseError::Transaction(
                "No database client available".to_string(),
            ))
        }
    }

    async fn commit(&mut self) -> Result<(), tokio_postgres::Error> {
        if let Some(tx) = self.transaction.take() {
            tx.commit().await?;
        }
        self.finalized = true;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), tokio_postgres::Error> {
        if let Some(tx) = self.transaction.take() {
            tx.rollback().await?;
        }
        self.finalized = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests would require a real database connection to be comprehensive
    // For now, we just test the basic state management

    #[tokio::test]
    async fn test_transaction_holder_creation() {
        let holder = TransactionHolder::new();
        assert!(!holder.state.lock().await.finalized);
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
        assert!(!holder.state.lock().await.finalized);

        // After finalize, should be finalized (even without actual DB operations)
        let _ = holder.finalize(true).await; // We expect this to succeed even without DB
        assert!(holder.state.lock().await.finalized);
    }
}
