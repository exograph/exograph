use deadpool_postgres::Transaction;

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

    // pub async fn create_transaction(&self) -> Result<Transaction<'_>, DatabaseError> {
    //     let mut client = self.database.get_client().await?;
    //     let client: &mut Client = &mut client;

    //     let tx = client.transaction().await?;

    //     Ok(tx)
    // }
}

#[derive(Default)]
pub struct TransactionHolder {
    client: Option<*mut deadpool_postgres::Client>,
    transaction: Option<*mut Transaction<'static>>,
}

unsafe impl Sync for TransactionHolder {}
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
                    let client = unsafe { self.client.map(|ptr| ptr.as_mut().unwrap()) }.unwrap();
                    let mut tx = Box::new(client.transaction().await?);
                    let res = work.execute(&mut tx).await;

                    self.transaction = Some(Box::leak(tx));

                    res
                }
            }
        }
    }

    pub async fn finalize(&mut self) -> Result<(), tokio_postgres::Error> {
        let tx_owned = unsafe {
            let mut tx_owned: Option<*mut Transaction> = None;
            std::mem::swap(&mut self.transaction, &mut tx_owned);
            tx_owned.map(|ptr| Box::from_raw(ptr))
        };

        match tx_owned {
            Some(boxed) => boxed.commit().await,

            None => Ok(()),
        }
    }
}
