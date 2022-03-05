use anyhow::{anyhow, Context, Result};
use tokio_postgres::{types::ToSql, Client, GenericClient, Row};

use crate::sql::ExpressionContext;

use super::{sql_operation::TemplateSQLOperation, OperationExpression, SQLOperation, SQLValue};

type TransactionStepResult = Vec<Row>;

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
    pub fn new(steps: Vec<TransactionStep<'a>>) -> Self {
        Self { steps }
    }
    
    /// Returns the result of the last step
    pub async fn execute(&'a self, client: &mut Client) -> Result<TransactionStepResult> {
        println!("Starting transaction");
        let mut tx = client.transaction().await?;

        let mut transaction_context = TransactionContext { results: vec![] };

        for step in self.steps.iter() {
            let result = step.execute(&mut tx, &transaction_context).await?;
            transaction_context.results.push(result)
        }

        println!("Committing transaction");
        tx.commit().await?;

        transaction_context
            .results
            .into_iter()
            .last()
            .ok_or_else(|| anyhow!(""))
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
    pub async fn execute(
        &self,
        client: &mut impl GenericClient,
        transaction_context: &TransactionContext,
    ) -> Result<TransactionStepResult> {
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

    pub async fn execute(
        &'a self,
        client: &mut impl GenericClient,
    ) -> Result<TransactionStepResult> {
        self.run_query(client).await
    }

    async fn run_query(&'a self, client: &mut impl GenericClient) -> Result<Vec<Row>> {
        let sql_operation = &self.operation;
        let mut context = ExpressionContext::default();
        let binding = sql_operation.binding(&mut context);

        let params: Vec<&(dyn ToSql + Sync)> =
            binding.params.iter().map(|p| (*p).as_pg()).collect();

        println!("Executing transaction step: {}", binding.stmt);
        client
            .query(binding.stmt.as_str(), &params[..])
            .await
            .context("PostgreSQL query failed")
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
