// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use tokio_postgres::{GenericClient, Row, Transaction};
use tracing::{debug, error, instrument};

use crate::{database_error::DatabaseError, sql::SQLBuilder, Database};

use super::{
    sql_operation::{SQLOperation, TemplateSQLOperation},
    ExpressionBuilder, SQLValue,
};

/// Rows obtained from a SQL operation
pub type TransactionStepResult = Vec<Row>;

/// Sequence of SQL operations that are executed in a transaction
#[derive(Default, Debug)]
pub struct TransactionScript<'a> {
    steps: Vec<TransactionStep<'a>>,
}

/// Collection of results from steps in a transaction
pub struct TransactionContext {
    results: Vec<TransactionStepResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransactionStepId(pub usize);

impl TransactionContext {
    /// Returns the value of a column in a row from the given step id
    pub fn resolve_value(&self, step_id: TransactionStepId, row: usize, col: usize) -> SQLValue {
        self.results[step_id.0][row].get::<usize, SQLValue>(col)
    }

    /// Returns the number of rows in the result of the given step id
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
        self,
        database: &Database,
        tx: &mut Transaction<'_>,
    ) -> Result<TransactionStepResult, DatabaseError> {
        let mut transaction_context = TransactionContext { results: vec![] };

        // Execute each step in the transaction and store the result in the transaction_context
        for step in self.steps.into_iter() {
            let result = step.execute(database, tx, &transaction_context).await?;
            transaction_context.results.push(result)
        }

        // Return the result of the last step (usually the "select")
        transaction_context
            .results
            .into_iter()
            .last()
            .ok_or_else(|| DatabaseError::Transaction("".into()))
    }

    /// Adds a step to the transaction script and return the step id (which is just the index of the step in the script)
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
        self,
        database: &Database,
        client: &mut impl GenericClient,
        transaction_context: &TransactionContext,
    ) -> Result<TransactionStepResult, DatabaseError> {
        match self {
            Self::Concrete(step) => step.execute(database, client).await,
            Self::Template(step) => {
                let concrete = step.resolve(transaction_context);

                let mut res: Result<TransactionStepResult, DatabaseError> = Ok(vec![]);

                let substep_count = concrete.len();

                for (index, substep) in concrete.into_iter().enumerate() {
                    if index == substep_count - 1 {
                        // Execute the last step and return the result
                        res = substep.execute(database, client).await;
                    } else {
                        // Execute all but the last step
                        substep.execute(database, client).await?;
                    }
                }

                res
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
        self,
        database: &Database,
        client: &mut impl GenericClient,
    ) -> Result<TransactionStepResult, DatabaseError> {
        self.run_query(database, client).await
    }

    async fn run_query(
        &'a self,
        database: &Database,
        client: &mut impl GenericClient,
    ) -> Result<Vec<Row>, DatabaseError> {
        let mut sql_builder = SQLBuilder::new();
        self.operation.build(database, &mut sql_builder);
        let (stmt, params) = sql_builder.into_sql();

        let params: Vec<_> = params.iter().map(|p| p.as_pg()).collect();

        debug!("Executing SQL operation: {}", stmt);

        client.query(&stmt, &params[..]).await.map_err(|e| {
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
