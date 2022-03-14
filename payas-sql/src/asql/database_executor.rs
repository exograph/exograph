use crate::{
    sql::{database::Database, transaction::TransactionStepResult},
    transform::{pg::Postgres, transformer::Transformer},
};
use anyhow::Result;

use super::abstract_operation::AbstractOperation;

pub struct DatabaseExecutor<'a> {
    pub database: &'a Database,
}

impl DatabaseExecutor<'_> {
    /// Execute an operation on a database.
    ///
    /// Currently makes a hard assumption on Postgres implementation, but this could be made more generic.
    pub async fn execute<'a>(
        &self,
        operation: &'a AbstractOperation<'a>,
    ) -> Result<TransactionStepResult> {
        let mut client = self.database.get_client().await?;

        let database_kind = Postgres {};

        let transaction_script = Transformer::to_transaction_script(&database_kind, operation);
        transaction_script.execute(&mut client).await
    }
}
