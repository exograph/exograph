use deadpool_postgres::Transaction;
use std::sync::atomic::AtomicBool;

use crate::{
    database_error::DatabaseError,
    sql::{
        database::Database,
        transaction::{TransactionScript, TransactionStepResult},
    },
    transform::{pg::Postgres, transformer::Transformer},
};

use super::abstract_operation::AbstractOperation;

pub struct DatabaseExecutor {
    pub database: Database,
}

impl DatabaseExecutor {
    /// Execute an operation on a database.
    ///
    /// Currently makes a hard assumption on Postgres implementation, but this could be made more generic.
    pub async fn execute<'a>(
        &self,
        operation: &'a AbstractOperation<'a>,
        tx_holder: &mut TransactionHolder,
    ) -> Result<TransactionStepResult, DatabaseError> {
        let database_kind = Postgres {};
        let transaction_script = database_kind.to_transaction_script(operation);

        tx_holder.with_tx(&self.database, &transaction_script).await
    }
}

// TransactionHolder holds raw pointers to two objects: `client` and `transaction`.
// `transaction` holds a reference to `client`, which makes initializing this struct properly difficult.
// In addition, we must interact with async methods when using either of these objects, further complicating things
// and preventing us from using libraries like self_cell and ouroboros.
//
// To simplify lifetime constraints, these are allocated and dropped manually through Box::leak
// and a manual Drop impl. By doing so, this grants `transaction` a 'static lifetime that oversteps some lifetime
// issues we encountered. Of course, we must manually make sure that the objects are tied to the lifetime of TransactionHolder.

#[derive(Default)]
pub struct TransactionHolder {
    client: Option<*mut deadpool_postgres::Client>,
    transaction: Option<*mut Transaction<'static>>,
    finalized: AtomicBool,
}

// needed to mark mut pointers in struct as Send
// https://internals.rust-lang.org/t/shouldnt-pointers-be-send-sync-or/8818/4
unsafe impl Send for TransactionHolder {}

impl Drop for TransactionHolder {
    fn drop(&mut self) {
        if let Some(client) = self.client {
            let client = unsafe { Box::from_raw(client) };
            drop(client)
        }

        if let Some(transaction) = self.transaction {
            let transaction = unsafe { Box::from_raw(transaction) };
            drop(transaction)
        }
    }
}

impl TransactionHolder {
    pub async fn with_tx(
        &mut self,
        database: &Database,
        work: &TransactionScript<'_>,
    ) -> Result<TransactionStepResult, DatabaseError> {
        if self.finalized.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(DatabaseError::Transaction(
                "Transaction already finalized".into(),
            ));
        }

        // this should be safe, we only really handle transaction in this function and it should
        // always be de-referencable when it is a Some(_)
        let tx = unsafe { self.transaction.map(|ptr| ptr.as_mut().unwrap()) };

        match tx {
            Some(tx) => work.execute(tx).await,

            None => {
                // first, grab a client if none are available
                {
                    let client_owned = unsafe {
                        let mut client_owned: Option<*mut deadpool_postgres::Client> = None;
                        std::mem::swap(&mut self.client, &mut client_owned);
                        client_owned.map(|ptr| Box::from_raw(ptr))
                    };

                    if client_owned.is_none() {
                        let client = database.get_client().await?;
                        self.client = Some(Box::leak(Box::new(client)));
                    };
                }

                // proceed with grabbing a transaction and execution
                {
                    // this should always be de-referencable when it is a Some(_)
                    let client = unsafe { self.client.map(|ptr| ptr.as_mut().unwrap()) }.unwrap();
                    let mut tx = Box::new(client.transaction().await?);
                    let res = work.execute(&mut tx).await;

                    self.transaction = Some(Box::leak(tx));

                    res
                }
            }
        }
    }

    pub async fn finalize(&mut self, commit: bool) -> Result<(), tokio_postgres::Error> {
        let tx_owned = unsafe {
            let mut tx_owned: Option<*mut Transaction> = None;
            std::mem::swap(&mut self.transaction, &mut tx_owned);
            tx_owned.map(|ptr| Box::from_raw(ptr))
        };

        match tx_owned {
            Some(boxed) => {
                if commit {
                    boxed.commit().await
                } else {
                    boxed.rollback().await
                }
            }

            None => Ok(()),
        }?;

        self.finalized
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}
