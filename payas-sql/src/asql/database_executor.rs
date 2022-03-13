use crate::sql::{database::Database, transaction::TransactionStepResult};
use anyhow::Result;

use super::abstract_operation::AbstractOperation;

pub struct DatabaseExecutor<'a> {
    pub database: &'a Database,
}

impl DatabaseExecutor<'_> {
    pub async fn execute<'a>(
        &self,
        abstract_operation: &'a AbstractOperation<'a>,
    ) -> Result<TransactionStepResult> {
        let mut client = self.database.get_client().await?;

        let transaction_script = abstract_operation.to_transaction_script();
        transaction_script.execute(&mut client).await
    }
}
