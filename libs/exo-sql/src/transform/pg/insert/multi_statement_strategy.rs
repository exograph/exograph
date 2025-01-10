// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use maybe_owned::MaybeOwned;

use crate::{
    sql::{
        column::{ArrayParamWrapper, ProxyColumn},
        insert::TemplateInsert,
        select::Select,
        sql_operation::{SQLOperation, TemplateSQLOperation},
        transaction::{
            ConcreteTransactionStep, DynamicTransactionStep, TemplateTransactionStep,
            TransactionContext, TransactionScript, TransactionStep, TransactionStepId,
        },
    },
    transform::{pg::Postgres, transformer::SelectTransformer},
    AbstractInsert, Column, ColumnId, ColumnValuePair, Database, InsertionRow, NestedInsertion,
    Predicate, SQLParamContainer, TableId,
};

use super::insertion_strategy::InsertionStrategy;

pub(crate) struct MultiStatementStrategy {}

/// Insertion strategy that uses multiple statements to insert rows.
///
/// For each row, we insert the row itself, and then insert any nested rows (and we do this recursively).
impl InsertionStrategy for MultiStatementStrategy {
    fn id(&self) -> &'static str {
        "MultiStatementStrategy"
    }

    fn suitable(&self, _abstract_insert: &AbstractInsert, _database: &Database) -> bool {
        true
    }

    fn update_transaction_script<'a>(
        &self,
        abstract_insert: &'a AbstractInsert,
        parent_step: Option<(TransactionStepId, Vec<ColumnId>)>,
        database: &'a Database,
        transformer: &Postgres,
        transaction_script: &mut TransactionScript<'a>,
    ) {
        let AbstractInsert {
            table_id,
            rows,
            selection,
        } = abstract_insert;

        let insert_step_ids: Vec<_> = rows
            .iter()
            .map(|row| {
                insert_row(
                    *table_id,
                    row,
                    parent_step.clone(),
                    transaction_script,
                    database,
                )
            })
            .collect();

        let select = transformer.to_select(selection, database);

        // Take the previous insert steps and use them as the input to the select
        // statement to form a predicate `pk IN (insert_step_1_pk, insert_step_2_pk, ...)`
        let select_transformation = Box::new(move |transaction_context: &TransactionContext| {
            let predicate = database
                .get_table(*table_id)
                .get_pk_column_indices()
                .into_iter()
                .enumerate()
                .fold(
                    select.predicate,
                    |predicate, (pk_column_index, pk_physical_column_index)| {
                        let pk_column_id = ColumnId {
                            table_id: *table_id,
                            column_index: pk_physical_column_index,
                        };
                        let pk_physical_column =
                            &database.get_table(*table_id).columns[pk_physical_column_index];

                        let in_values = SQLParamContainer::from_sql_values(
                            insert_step_ids
                                .clone()
                                .into_iter()
                                .map(|insert_step_id| {
                                    transaction_context.resolve_value(
                                        insert_step_id,
                                        0,
                                        pk_column_index,
                                    )
                                })
                                .collect::<Vec<_>>(),
                            pk_physical_column.typ.get_pg_type(),
                        );

                        Predicate::and(
                            predicate,
                            Predicate::Eq(
                                Column::physical(pk_column_id, None),
                                Column::ArrayParam {
                                    param: in_values,
                                    wrapper: ArrayParamWrapper::Any,
                                },
                            ),
                        )
                    },
                );

            ConcreteTransactionStep::new(SQLOperation::Select(Select {
                predicate,
                ..select
            }))
        });

        transaction_script.add_step(TransactionStep::Dynamic(DynamicTransactionStep {
            function: select_transformation,
        }));
    }
}

fn insert_row<'a>(
    table_id: TableId,
    row: &'a InsertionRow,
    parent_step: Option<(TransactionStepId, Vec<ColumnId>)>,
    transaction_script: &mut TransactionScript<'a>,
    database: &'a Database,
) -> TransactionStepId {
    let (self_row, nested_rows) = row.partition_self_and_nested();

    let self_insert_id = insert_self_row(
        table_id,
        self_row,
        parent_step,
        transaction_script,
        database,
    );

    for nested_row in nested_rows {
        insert_nested_row(nested_row, self_insert_id, transaction_script, database);
    }

    self_insert_id
}

fn insert_self_row<'a>(
    table_id: TableId,
    row: Vec<&'a ColumnValuePair>,
    parent_step: Option<(TransactionStepId, Vec<ColumnId>)>,
    transaction_script: &mut TransactionScript<'a>,
    database: &'a Database,
) -> TransactionStepId {
    let table = database.get_table(table_id);

    let (mut columns, values): (Vec<_>, Vec<_>) = row
        .into_iter()
        .map(|ColumnValuePair { column, value }| {
            (column.get_column(database), MaybeOwned::Borrowed(value))
        })
        .unzip();

    let pk_column_ids = database.get_pk_column_ids(table_id);

    let returning = pk_column_ids
        .into_iter()
        .map(|column_id| Column::physical(column_id, None))
        .collect();

    match parent_step {
        Some((parent_step_id, parent_column_ids)) => {
            for parent_column_id in parent_column_ids.iter() {
                columns.push(parent_column_id.get_column(database));
            }
            let mut proxy_values = values
                .into_iter()
                .map(ProxyColumn::Concrete)
                .collect::<Vec<_>>();

            for col_index in 0..parent_column_ids.len() {
                proxy_values.push(ProxyColumn::Template {
                    col_index,
                    step_id: parent_step_id,
                });
            }

            let insert = TemplateSQLOperation::Insert(TemplateInsert {
                table,
                columns,
                column_values_seq: vec![proxy_values],
                returning,
            });
            transaction_script.add_step(TransactionStep::Template(TemplateTransactionStep {
                operation: insert,
                prev_step_id: parent_step_id,
            }))
        }
        None => {
            let insert = SQLOperation::Insert(table.insert(
                columns,
                vec![values],
                returning.into_iter().map(|c| c.into()).collect(),
            ));
            transaction_script.add_step(TransactionStep::Concrete(ConcreteTransactionStep::new(
                insert,
            )))
        }
    }
}

fn insert_nested_row<'a>(
    nested_row: &'a NestedInsertion,
    parent_step_id: TransactionStepId,
    transaction_script: &mut TransactionScript<'a>,
    database: &'a Database,
) {
    let NestedInsertion {
        relation_id,
        insertions,
    } = nested_row;

    let relation = relation_id.deref(database);
    let foreign_table_id = relation.linked_table_id;
    let foreign_column_ids = relation
        .column_pairs
        .iter()
        .map(|pair| pair.foreign_column_id)
        .collect::<Vec<_>>();

    for insertion in insertions {
        insert_row(
            foreign_table_id,
            insertion,
            Some((parent_step_id, foreign_column_ids.clone())),
            transaction_script,
            database,
        );
    }
}
