// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::fmt::Debug;

use tokio_postgres::Row;

use exo_sql_core::{Database, Predicate, SQLParamContainer, TableId};

use crate::{
    column::{ArrayParamWrapper, Column},
    predicate::ConcretePredicate,
    select::Select,
    sql_operation::{SQLOperation, TemplateSQLOperation},
    table::Table,
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

impl Default for TransactionContext {
    fn default() -> Self {
        Self::new()
    }
}

impl TransactionContext {
    pub fn new() -> Self {
        Self { results: vec![] }
    }

    pub fn push(&mut self, result: TransactionStepResult) {
        self.results.push(result);
    }

    pub fn into_last_result(self) -> Option<TransactionStepResult> {
        self.results.into_iter().next_back()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransactionStepId(pub usize);

impl TransactionContext {
    /// Returns the value of a column in a row from the given step id
    pub fn resolve_value(
        &self,
        step_id: TransactionStepId,
        row: usize,
        col: usize,
    ) -> exo_sql_core::sql_value::SQLValue {
        self.results[step_id.0][row].get::<usize, exo_sql_core::sql_value::SQLValue>(col)
    }

    /// Returns the number of rows in the result of the given step id
    pub fn row_count(&self, step_id: TransactionStepId) -> usize {
        self.results[step_id.0].len()
    }
}

impl<'a> TransactionScript<'a> {
    /// Adds a step to the transaction script and return the step id (which is just the index of the step in the script)
    pub fn add_step(&mut self, step: TransactionStep<'a>) -> TransactionStepId {
        let id = self.steps.len();
        self.steps.push(step);
        TransactionStepId(id)
    }

    pub fn needs_transaction(&self) -> bool {
        self.steps.len() > 1
    }

    /// Consume the script and return the steps
    pub fn into_steps(self) -> Vec<TransactionStep<'a>> {
        self.steps
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

#[derive(Debug)]
pub struct ConcreteTransactionStep<'a> {
    pub operation: SQLOperation<'a>,
}

impl<'a> ConcreteTransactionStep<'a> {
    pub fn new(operation: SQLOperation<'a>) -> Self {
        Self { operation }
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
