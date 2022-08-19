use tokio_postgres::{types::ToSql, GenericClient, Row, Transaction};
use tracing::{error, instrument};

use crate::{database_error::DatabaseError, sql::ExpressionContext};

use super::{
    sql_operation::{SQLOperation, TemplateSQLOperation},
    OperationExpression, SQLValue,
};

pub type TransactionStepResult = Vec<Row>;

#[derive(Default, Debug)]
pub struct TransactionScript<'a> {
    steps: Vec<TransactionStep<'a>>,
}

pub struct TransactionContext {
    results: Vec<TransactionStepResult>,
}

#[derive(Debug, Clone, Copy)]
pub struct TransactionStepId(pub usize);

impl TransactionContext {
    pub fn resolve_value(&self, step_id: TransactionStepId, row: usize, col: usize) -> SQLValue {
        self.results
            .get(step_id.0)
            .unwrap()
            .get(row)
            .unwrap()
            .try_get::<usize, SQLValue>(col)
            .unwrap()
    }

    pub fn row_count(&self, step_id: TransactionStepId) -> usize {
        self.results[step_id.0].len()
    }
}

impl<'a> TransactionScript<'a> {
    /// Returns the result of the last step
    #[instrument(
        name = "TransactionScript::execute"
        skip(self, tx)
        )]
    pub async fn execute(
        &self,
        tx: &mut Transaction<'_>,
    ) -> Result<TransactionStepResult, DatabaseError> {
        let mut transaction_context = TransactionContext { results: vec![] };

        for step in self.steps.iter() {
            let result = step.execute(tx, &transaction_context).await?;
            transaction_context.results.push(result)
        }

        transaction_context
            .results
            .into_iter()
            .last()
            .ok_or_else(|| DatabaseError::Transaction("".into()))
    }

    pub fn add_step(&mut self, step: TransactionStep<'a>) -> TransactionStepId {
        let id = self.steps.len();
        self.steps.push(step);
        TransactionStepId(id)
    }
}

#[derive(Debug)]
pub enum TransactionStep<'a> {
    Concrete(ConcreteTransactionStep<'a>),
    Template(TemplateTransactionStep<'a>),
}

impl<'a> TransactionStep<'a> {
    #[instrument(
        name = "TransactionStep::execute"
        level = "trace"
        skip(self, client, transaction_context)
        )]
    pub async fn execute(
        &self,
        client: &mut impl GenericClient,
        transaction_context: &TransactionContext,
    ) -> Result<TransactionStepResult, DatabaseError> {
        match self {
            Self::Concrete(step) => step.execute(client).await,
            Self::Template(step) => {
                let concrete = step.resolve(transaction_context);

                match concrete.as_slice() {
                    [init @ .., last] => {
                        for substep in init {
                            substep.execute(client).await?;
                        }
                        last.execute(client).await
                    }
                    _ => Ok(vec![]),
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct ConcreteTransactionStep<'a> {
    pub operation: SQLOperation<'a>,
}

impl<'a> ConcreteTransactionStep<'a> {
    pub fn new(operation: SQLOperation<'a>) -> Self {
        Self { operation }
    }

    #[instrument(
        name = "ConcreteTransactionStep::execute"
        level = "trace"
        skip(self, client)
        fields(
            operation = ?self.operation
            )
        )]
    pub async fn execute(
        &'a self,
        client: &mut impl GenericClient,
    ) -> Result<TransactionStepResult, DatabaseError> {
        self.run_query(client).await
    }

    async fn run_query(
        &'a self,
        client: &mut impl GenericClient,
    ) -> Result<Vec<Row>, DatabaseError> {
        let sql_operation = &self.operation;
        let mut context = ExpressionContext::default();
        let binding = sql_operation.binding(&mut context);

        let params: Vec<&(dyn ToSql + Sync)> =
            binding.params.iter().map(|p| (*p).as_pg()).collect();

        client
            .query(binding.stmt.as_str(), &params[..])
            .await
            .map_err(|e| {
                error!("Failed to execute query: {e:?}");
                DatabaseError::Delegate(e).with_context("Database operation failed".into())
            })
    }
}

#[derive(Debug)]
pub struct TemplateTransactionStep<'a> {
    pub operation: TemplateSQLOperation<'a>,
    pub prev_step_id: TransactionStepId,
}

impl<'a> TemplateTransactionStep<'a> {
    pub fn resolve(
        &'a self,
        transaction_context: &TransactionContext,
    ) -> Vec<ConcreteTransactionStep<'a>> {
        self.operation
            .resolve(self.prev_step_id, transaction_context)
            .into_iter()
            .map(|operation| ConcreteTransactionStep { operation })
            .collect()
    }
}
