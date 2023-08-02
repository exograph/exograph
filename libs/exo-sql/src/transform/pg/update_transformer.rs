// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Transform an abstract update into a transaction script.
//!
//! This allows us to execute GraphQL mutations like this:
//!
//! ```graphql
//! mutation {
//!   updateConcert(id: 4, data: {
//!     title: "new-title",
//!     concertArtists: {
//!       create: [{artist: {id: 30}, rank: 2, role: "main"}],
//!       update: [{id: 100, artist: {id: 10}, rank: 2}, {id: 101, artist: {id: 10}, role: "accompanying"}],
//!       delete: [{id: 110}]
//!     }
//!   }) {
//!     id
//!   }
//! }
//! ```
//!
use maybe_owned::MaybeOwned;
use tracing::instrument;

use crate::{
    asql::update::{
        AbstractUpdate, NestedAbstractDelete, NestedAbstractInsert, NestedAbstractUpdate,
    },
    sql::{
        column::{Column, ProxyColumn},
        cte::{CteExpression, WithQuery},
        delete::TemplateDelete,
        insert::TemplateInsert,
        sql_operation::{SQLOperation, TemplateSQLOperation},
        transaction::{
            ConcreteTransactionStep, TemplateFilterOperation, TemplateTransactionStep,
            TransactionScript, TransactionStep, TransactionStepId,
        },
        update::TemplateUpdate,
    },
    transform::transformer::{PredicateTransformer, SelectTransformer, UpdateTransformer},
    ColumnId, Database, NestedAbstractInsertSet, PhysicalColumn,
};

use super::{selection_level::SelectionLevel, Postgres};

impl UpdateTransformer for Postgres {
    /// Transform an abstract update into a transaction script.
    /// Created transaction script looks like (for the example above):
    /// ```sql
    /// UPDATE "concerts" SET "title" = $1 WHERE "concerts"."id" = $2 RETURNING "concerts"."id"
    /// UPDATE "concert_artists" SET "artist_id" = $1, "rank" = $2, "concert_id" = $3 WHERE "concert_artists"."id" = $4
    /// DELETE FROM "concert_artists" WHERE "concert_artists"."id" = $1
    /// SELECT json_build_object('id', "concerts"."id")::text FROM "concerts" WHERE "concerts"."id" = $1
    /// ```
    #[instrument(
        name = "UpdateTransformer::to_transaction_script for Postgres"
        skip(self)
        )]
    fn to_transaction_script<'a>(
        &self,
        abstract_update: &'a AbstractUpdate,
        database: &'a Database,
    ) -> TransactionScript<'a> {
        let column_id_values: Vec<(ColumnId, MaybeOwned<'a, Column>)> = abstract_update
            .column_values
            .iter()
            .map(|(c, v)| (*c, v.into()))
            .collect();

        let predicate = self.to_predicate(
            &abstract_update.predicate,
            &SelectionLevel::TopLevel,
            false,
            database,
        );

        let select = self.to_select(&abstract_update.selection, database);

        let has_nested_ops = !abstract_update.nested_updates.is_empty()
            || !abstract_update.nested_inserts.is_empty()
            || !abstract_update.nested_deletes.is_empty();

        // If there is no nested update, select all the columns, so that the select statement in the
        // CTE will have all those column (and not have to specify the WHERE clause once again).
        // If there are nested updates, select only the primary key column, so that we can use that
        // as the proxy column in the nested updates added to the transaction script.
        let return_col = if has_nested_ops {
            Column::physical(
                database
                    .get_pk_column_id(abstract_update.table_id)
                    .expect("No primary key column"),
                None,
            )
        } else {
            Column::Star(None)
        };

        let table = database.get_table(abstract_update.table_id);
        let column_values = column_id_values
            .into_iter()
            .map(|(col_id, col)| (col_id.get_column(database), col))
            .collect();
        let root_update = SQLOperation::Update(table.update(
            column_values,
            predicate.into(),
            vec![return_col.into()],
        ));

        let mut transaction_script = TransactionScript::default();

        if has_nested_ops {
            let root_step_id = transaction_script.add_step(TransactionStep::Concrete(
                ConcreteTransactionStep::new(root_update),
            ));

            abstract_update
                .nested_updates
                .iter()
                .for_each(|nested_update| {
                    // Create a template update operation and bind it to the root step
                    let update_op = TemplateTransactionStep {
                        operation: update_op(nested_update, self, database),
                        prev_step_id: root_step_id,
                    };

                    let _ = transaction_script.add_step(TransactionStep::Template(update_op));
                });

            abstract_update.nested_inserts.iter().for_each(
                |NestedAbstractInsertSet {
                     ops,
                     filter_predicate,
                 }| {
                    let filter_step_predicate = self.to_predicate(
                        filter_predicate,
                        &SelectionLevel::TopLevel,
                        false,
                        database,
                    );

                    let filter_step_id = transaction_script.add_step(TransactionStep::Filter(
                        TemplateFilterOperation {
                            table_id: abstract_update.table_id,
                            prev_step_id: root_step_id,
                            predicate: filter_step_predicate,
                        },
                    ));

                    for nested_insert in ops {
                        let insert_op = TemplateTransactionStep {
                            operation: insert_op(nested_insert, filter_step_id, database),
                            prev_step_id: filter_step_id,
                        };

                        let _ = transaction_script.add_step(TransactionStep::Template(insert_op));
                    }
                },
            );

            abstract_update
                .nested_deletes
                .iter()
                .for_each(|nested_delete| {
                    let delete_op = TemplateTransactionStep {
                        operation: delete_op(nested_delete, self, database),
                        prev_step_id: root_step_id,
                    };

                    let _ = transaction_script.add_step(TransactionStep::Template(delete_op));
                });

            let _ = transaction_script.add_step(TransactionStep::Concrete(
                ConcreteTransactionStep::new(SQLOperation::Select(select)),
            ));
        } else {
            // Simpler case where we do not have any nested insert/update/delete. We can just return a simple
            // CTE like:
            // ```sql
            // WITH "concerts" AS (
            //    UPDATE "concerts" SET "title" = $1 WHERE "concerts"."id" = $2 RETURNING *
            // )
            // SELECT json_build_object('id', "concerts"."id")::text FROM "concerts" WHERE "concerts"."id" = $3
            // ```
            transaction_script.add_step(TransactionStep::Concrete(ConcreteTransactionStep::new(
                SQLOperation::WithQuery(WithQuery {
                    expressions: vec![CteExpression {
                        name: table.name.clone(),
                        operation: root_update,
                    }],
                    select,
                }),
            )));
        }

        transaction_script
    }
}

fn update_op<'a>(
    nested_update: &'a NestedAbstractUpdate,
    predicate_transformer: &impl PredicateTransformer,
    database: &'a Database,
) -> TemplateSQLOperation<'a> {
    let column_values: Vec<(&PhysicalColumn, &Column)> = nested_update
        .update
        .column_values
        .iter()
        .map(|(col_id, col)| (col_id.get_column(database), col))
        .collect();

    TemplateSQLOperation::Update(TemplateUpdate {
        table: database.get_table(nested_update.update.table_id),
        predicate: predicate_transformer.to_predicate(
            &nested_update.update.predicate,
            &SelectionLevel::TopLevel,
            false,
            database,
        ),
        nesting_relation: nested_update.nesting_relation,
        column_values,
        returning: vec![],
    })
}

fn insert_op<'a>(
    nested_insert: &'a NestedAbstractInsert,
    parent_step_id: TransactionStepId,
    database: &'a Database,
) -> TemplateSQLOperation<'a> {
    let rows = &nested_insert.insert.rows;

    // TODO: Deal with _nested_elems (i.e. a recursive use of nested insert)
    let (self_elems, _nested_elems): (Vec<_>, Vec<_>) = rows
        .iter()
        .map(|row| row.partition_self_and_nested())
        .unzip();

    let (mut column_id_names, column_values_seq) = super::insert_transformer::align(self_elems);
    column_id_names.push(nested_insert.relation_column_id);

    let column_values_seq: Vec<_> = column_values_seq
        .into_iter()
        .map(|column_values| {
            let mut column_values: Vec<_> = column_values
                .into_iter()
                .map(ProxyColumn::Concrete)
                .collect();
            column_values.push(ProxyColumn::Template {
                // The column index is always 0 because we want to get the only column in the row
                // such as `UPDATE "concerts" SET "title" = $1 WHERE "concerts"."id" = $2 RETURNING
                // "concerts"."id"`
                col_index: 0,
                step_id: parent_step_id,
            });
            column_values
        })
        .collect();

    let columns = column_id_names
        .iter()
        .map(|col_id| col_id.get_column(database))
        .collect();

    TemplateSQLOperation::Insert(TemplateInsert {
        table: database.get_table(nested_insert.insert.table_id),
        columns,
        column_values_seq,
        returning: vec![],
    })
}

fn delete_op<'a>(
    nested_delete: &'a NestedAbstractDelete,
    predicate_transformer: &impl PredicateTransformer,
    database: &'a Database,
) -> TemplateSQLOperation<'a> {
    let predicate = predicate_transformer.to_predicate(
        &nested_delete.delete.predicate,
        &SelectionLevel::TopLevel,
        false,
        database,
    );

    TemplateSQLOperation::Delete(TemplateDelete {
        table: database.get_table(nested_delete.delete.table_id),
        predicate,
        nesting_relation: nested_delete.nesting_relation,
        returning: vec![],
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        asql::{
            column_path::ColumnPath,
            predicate::AbstractPredicate,
            select::AbstractSelect,
            selection::{AliasedSelectionElement, Selection, SelectionElement},
            update::NestedAbstractUpdate,
        },
        sql::{column::Column, predicate::Predicate, SQLParamContainer},
        transform::test_util::TestSetup,
        PhysicalColumnPath,
    };

    use super::*;

    #[test]
    fn simple_update() {
        TestSetup::with_setup(
            |TestSetup {
                 database,
                 venues_table,
                 venues_id_column,
                 venues_name_column,
                 ..
             }| {
                let venue_id_path =
                    ColumnPath::Physical(PhysicalColumnPath::leaf(venues_id_column));
                let literal = ColumnPath::Param(SQLParamContainer::new(5));
                let predicate = AbstractPredicate::eq(venue_id_path, literal);

                let abs_update = AbstractUpdate {
                    table_id: venues_table,
                    predicate,
                    column_values: vec![(
                        venues_name_column,
                        Column::Param(SQLParamContainer::new("new_name".to_string())),
                    )],
                    nested_updates: vec![],
                    nested_inserts: vec![],
                    nested_deletes: vec![],
                    selection: AbstractSelect {
                        table_id: venues_table,
                        selection: Selection::Seq(vec![
                            AliasedSelectionElement::new(
                                "id".to_string(),
                                SelectionElement::Physical(venues_id_column),
                            ),
                            AliasedSelectionElement::new(
                                "name".to_string(),
                                SelectionElement::Physical(venues_name_column),
                            ),
                        ]),
                        predicate: Predicate::True,
                        order_by: None,
                        offset: None,
                        limit: None,
                    },
                };

                let update =
                    UpdateTransformer::to_transaction_script(&Postgres {}, &abs_update, &database);

                // TODO: Add a proper assertion here (ideally, we can get a digest of the transaction script and assert on it)
                println!("{update:#?}");
            },
        )
    }

    #[test]
    fn nested_update() {
        TestSetup::with_setup(
            |TestSetup {
                 database,
                 venues_table,
                 venues_id_column,
                 venues_name_column,
                 concerts_table,
                 concerts_name_column,
                 concerts_venue_id_column,
                 ..
             }| {
                let venue_id_path =
                    ColumnPath::Physical(PhysicalColumnPath::leaf(venues_id_column));

                let literal = ColumnPath::Param(SQLParamContainer::new(5));
                let predicate = AbstractPredicate::eq(venue_id_path, literal);

                let nested_abs_update = NestedAbstractUpdate {
                    nesting_relation: concerts_venue_id_column
                        .get_otm_relation(&database)
                        .unwrap()
                        .deref(&database),
                    update: AbstractUpdate {
                        table_id: concerts_table,
                        predicate: Predicate::True,
                        column_values: vec![(
                            concerts_name_column,
                            Column::Param(SQLParamContainer::new("new_concert_name".to_string())),
                        )],
                        selection: AbstractSelect {
                            table_id: venues_table,
                            selection: Selection::Seq(vec![]),
                            predicate: Predicate::True,
                            order_by: None,
                            offset: None,
                            limit: None,
                        },
                        nested_updates: vec![],
                        nested_inserts: vec![],
                        nested_deletes: vec![],
                    },
                };

                let abs_update = AbstractUpdate {
                    table_id: venues_table,
                    predicate,
                    column_values: vec![(
                        venues_name_column,
                        Column::Param(SQLParamContainer::new("new_name".to_string())),
                    )],
                    nested_updates: vec![nested_abs_update],
                    nested_inserts: vec![],
                    nested_deletes: vec![],
                    selection: AbstractSelect {
                        table_id: venues_table,
                        selection: Selection::Seq(vec![
                            AliasedSelectionElement::new(
                                "id".to_string(),
                                SelectionElement::Physical(venues_id_column),
                            ),
                            AliasedSelectionElement::new(
                                "name".to_string(),
                                SelectionElement::Physical(venues_name_column),
                            ),
                        ]),
                        predicate: Predicate::True,
                        order_by: None,
                        offset: None,
                        limit: None,
                    },
                };

                let update =
                    UpdateTransformer::to_transaction_script(&Postgres {}, &abs_update, &database);

                // TODO: Add a proper assertion here (ideally, we can get a digest of the transaction script and assert on it)
                println!("{update:#?}");
            },
        )
    }
}
