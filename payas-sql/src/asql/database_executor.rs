use tokio_postgres::Client;

use crate::{
    database_error::DatabaseError,
    sql::{database::Database, transaction::TransactionStepResult},
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
    ) -> Result<TransactionStepResult, DatabaseError> {
        let mut client = self.database.get_client().await?;
        let client: &mut Client = &mut client;

        let database_kind = Postgres {};

        let transaction_script = database_kind.to_transaction_script(operation);

        let mut tx = client.transaction().await?;
        let result = transaction_script.execute(&mut tx).await;
        tx.commit().await?;

        result
    }
}
