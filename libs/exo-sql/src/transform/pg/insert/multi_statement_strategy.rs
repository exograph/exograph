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
    OneToMany, Predicate, SQLParamContainer, TableId,
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
        parent_step: Option<(TransactionStepId, ColumnId)>,
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
            .map(|row| insert_row(*table_id, row, parent_step, transaction_script, database))
            .collect();

        let select = transformer.to_select(selection, database);

        let pk_column_types = database
            .get_table(*table_id)
            .get_pk_physical_columns()
            .iter()
            .map(|pk_physical_column| pk_physical_column.typ.get_pg_type())
            .collect::<Vec<_>>();

        // Take the previous insert steps and use them as the input to the select
        // statement to form a predicate `pk IN (insert_step_1_pk, insert_step_2_pk, ...)`
        let select_transformation = Box::new(move |transaction_context: &TransactionContext| {
            let in_values = SQLParamContainer::from_sql_values(
                insert_step_ids
                    .into_iter()
                    .map(|insert_step_id| transaction_context.resolve_value(insert_step_id, 0, 0))
                    .collect::<Vec<_>>(),
                pk_column_types[0].clone(),
            );

            let predicate = Predicate::and(
                Predicate::Eq(
                    Column::physical(database.get_pk_column_ids(*table_id).remove(0), None),
                    Column::ArrayParam {
                        param: in_values,
                        wrapper: ArrayParamWrapper::Any,
                    },
                ),
                select.predicate,
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
    parent_step: Option<(TransactionStepId, ColumnId)>,
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
    parent_step: Option<(TransactionStepId, ColumnId)>,
    transaction_script: &mut TransactionScript<'a>,
    database: &'a Database,
) -> TransactionStepId {
    let pk_column = Column::physical(database.get_pk_column_ids(table_id).remove(0), None);

    let table = database.get_table(table_id);

    let (mut columns, values): (Vec<_>, Vec<_>) = row
        .into_iter()
        .map(|ColumnValuePair { column, value }| {
            (column.get_column(database), MaybeOwned::Borrowed(value))
        })
        .unzip();

    match parent_step {
        Some((parent_step_id, parent_column_id)) => {
            columns.push(parent_column_id.get_column(database));
            let mut proxy_values = values
                .into_iter()
                .map(ProxyColumn::Concrete)
                .collect::<Vec<_>>();
            proxy_values.push(ProxyColumn::Template {
                col_index: 0,
                step_id: parent_step_id,
            });

            let insert = TemplateSQLOperation::Insert(TemplateInsert {
                table,
                columns,
                column_values_seq: vec![proxy_values],
                returning: vec![pk_column],
            });
            transaction_script.add_step(TransactionStep::Template(TemplateTransactionStep {
                operation: insert,
                prev_step_id: parent_step_id,
            }))
        }
        None => {
            let insert =
                SQLOperation::Insert(table.insert(columns, vec![values], vec![pk_column.into()]));
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

    let OneToMany { column_pairs } = relation_id.deref(database);

    let foreign_column_id = column_pairs[0].foreign_column_id;
    for insertion in insertions {
        insert_row(
            foreign_column_id.table_id,
            insertion,
            Some((parent_step_id, foreign_column_id)),
            transaction_script,
            database,
        );
    }
}
