// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::fmt::Debug;

use tokio_postgres::{GenericClient, Row};
use tracing::{error, info, instrument};

use crate::{
    Column, Database, Predicate, SQLParamContainer, TableId,
    database_error::DatabaseError,
    sql::{SQLBuilder, select::Select, table::Table},
};

use super::{
    ExpressionBuilder, SQLValue,
    column::ArrayParamWrapper,
    predicate::ConcretePredicate,
    sql_operation::{SQLOperation, TemplateSQLOperation},
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
        skip_all
        )]
    pub async fn execute<T: tokio_postgres::GenericClient>(
        self,
        database: &Database,
        tx: &mut T,
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
            .next_back()
            .ok_or_else(|| DatabaseError::Transaction("".into()))
    }

    /// Adds a step to the transaction script and return the step id (which is just the index of the step in the script)
    pub fn add_step(&mut self, step: TransactionStep<'a>) -> TransactionStepId {
        let id = self.steps.len();
        self.steps.push(step);
        TransactionStepId(id)
    }

    pub fn needs_transaction(&self) -> bool {
        self.steps.len() > 1
    }
}

#[derive(Debug)]
pub enum TransactionStep<'a> {
    Concrete(Box<ConcreteTransactionStep<'a>>),
    Template(TemplateTransactionStep<'a>),
    Filter(TemplateFilterOperation),
    Dynamic(DynamicTransactionStep<'a>),
    Precheck(Select),
}

impl TransactionStep<'_> {
    #[instrument(
        name = "TransactionStep::execute"
        level = "trace"
        skip_all
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
            Self::Filter(step) => {
                let concrete = step.resolve(transaction_context, database);
                concrete.execute(database, client).await
            }
            Self::Dynamic(step) => {
                step.resolve(transaction_context)
                    .execute(database, client)
                    .await
            }
            Self::Precheck(select) => {
                let precheck_result =
                    run_query(SQLOperation::Select(select), database, client).await?;
                if precheck_result.len() != 1 {
                    return Err(DatabaseError::Precheck(format!(
                        "Expected 1 row, got {}",
                        precheck_result.len()
                    )));
                }

                Ok(precheck_result)
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
        skip_all
        fields(
            operation = ?self.operation
            )
        )]
    pub async fn execute(
        self,
        database: &Database,
        client: &mut impl GenericClient,
    ) -> Result<TransactionStepResult, DatabaseError> {
        run_query(self.operation, database, client).await
    }
}

async fn run_query(
    operation: SQLOperation<'_>,
    database: &Database,
    client: &mut impl GenericClient,
) -> Result<TransactionStepResult, DatabaseError> {
    let mut sql_builder = SQLBuilder::new();
    operation.build(database, &mut sql_builder);
    let (stmt, params) = sql_builder.into_sql();

    let params: Vec<_> = params
        .iter()
        .map(|p| (p.param.as_pg(), p.param_type.clone()))
        .collect();

    info!("Executing SQL operation: {}", stmt);
    println!("[debug] SQL: {}", stmt);
    println!(
        "[debug] Params: {:?}",
        params.iter().map(|(p, _)| p).collect::<Vec<_>>()
    );

    client.query_typed(&stmt, &params[..]).await.map_err(|e| {
        error!("Failed to execute query: {e:?}");
        DatabaseError::Delegate(e).with_context("Database operation failed".into())
    })
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

#[derive(Debug)]
pub struct TemplateFilterOperation {
    pub prev_step_id: TransactionStepId,
    pub table_id: TableId,
    pub predicate: ConcretePredicate,
}

impl TemplateFilterOperation {
    pub fn resolve<'a>(
        self,
        transaction_context: &TransactionContext,
        database: &Database,
    ) -> ConcreteTransactionStep<'a> {
        let rows = transaction_context.row_count(self.prev_step_id);

        let pk_column_ids = database.get_pk_column_ids(self.table_id);
        let pk_column_types = database
            .get_table(self.table_id)
            .get_pk_physical_columns()
            .iter()
            .map(|pk_physical_column| pk_physical_column.typ.get_pg_type())
            .collect::<Vec<_>>();

        let predicate = pk_column_ids.iter().enumerate().fold(
            self.predicate,
            |predicate, (index, pk_column_id)| {
                Predicate::and(
                    predicate,
                    Predicate::Eq(
                        Column::physical(*pk_column_id, None),
                        Column::ArrayParam {
                            param: SQLParamContainer::from_sql_values(
                                (0..rows)
                                    .map(|row| {
                                        transaction_context.resolve_value(
                                            self.prev_step_id,
                                            row,
                                            index,
                                        )
                                    })
                                    .collect::<Vec<_>>(),
                                pk_column_types[index].clone(),
                            ),
                            wrapper: ArrayParamWrapper::Any,
                        },
                    ),
                )
            },
        );

        ConcreteTransactionStep {
            operation: SQLOperation::Select(Select {
                table: Table::physical(self.table_id, None),
                predicate,
                order_by: None,
                offset: None,
                limit: None,
                top_level_selection: false,
                columns: pk_column_ids
                    .into_iter()
                    .map(|pk_column_id| Column::physical(pk_column_id, None))
                    .collect(),
                group_by: None,
            }),
        }
    }
}

/// A step that is resolved at runtime (e.g. a select that depends on the result of a previous step)
pub struct DynamicTransactionStep<'a> {
    pub function: Box<dyn FnOnce(&TransactionContext) -> ConcreteTransactionStep<'a> + Send + 'a>,
}

impl<'a> DynamicTransactionStep<'a> {
    pub fn resolve(self, transaction_context: &TransactionContext) -> ConcreteTransactionStep<'a> {
        (self.function)(transaction_context)
    }
}

impl std::fmt::Debug for DynamicTransactionStep<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicTransactionStep").finish()
    }
}
