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
    ColumnId, NestedAbstractDelete, NestedAbstractInsert, NestedAbstractInsertSet,
    NestedAbstractUpdate, PhysicalColumn, Predicate, SQLParamContainer,
    sql::{
        column::ArrayParamWrapper,
        delete::TemplateDelete,
        select::Select,
        sql_operation::TemplateSQLOperation,
        table::Table,
        transaction::{
            DynamicTransactionStep, TemplateFilterOperation, TemplateTransactionStep,
            TransactionContext, TransactionStepId,
        },
        update::TemplateUpdate,
    },
    transform::{
        pg::precheck::add_precheck_queries,
        transformer::{InsertTransformer, PredicateTransformer},
    },
};

use crate::{
    AbstractUpdate, Column, Database,
    sql::{
        sql_operation::SQLOperation,
        transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep},
    },
    transform::{
        pg::{Postgres, selection_level::SelectionLevel},
        transformer::SelectTransformer,
    },
};

use super::update_strategy::UpdateStrategy;

pub(crate) struct MultiStatementStrategy {}

/// Transform an abstract update into a transaction script.
/// Created transaction script looks like (for the example above):
/// ```sql
/// UPDATE "concerts" SET "title" = $1 WHERE "concerts"."id" = $2 RETURNING "concerts"."id"
/// UPDATE "concert_artists" SET "artist_id" = $1, "rank" = $2, "concert_id" = $3 WHERE "concert_artists"."id" = $4
/// DELETE FROM "concert_artists" WHERE "concert_artists"."id" = $1
/// SELECT json_build_object('id', "concerts"."id")::text FROM "concerts" WHERE "concerts"."id" = $1
/// ```
impl UpdateStrategy for MultiStatementStrategy {
    fn id(&self) -> &'static str {
        "MultiStatementStrategy"
    }

    fn suitable(&self, _abstract_update: &AbstractUpdate, _database: &Database) -> bool {
        true
    }

    /// Transform an abstract update into a transaction script.
    /// Created transaction script looks like (for the example above):
    /// ```sql
    /// UPDATE "concerts" SET "title" = $1 WHERE "concerts"."id" = $2 RETURNING "concerts"."id"
    /// UPDATE "concert_artists" SET "artist_id" = $1, "rank" = $2, "concert_id" = $3 WHERE "concert_artists"."id" = $4
    /// DELETE FROM "concert_artists" WHERE "concert_artists"."id" = $1
    /// SELECT json_build_object('id', "concerts"."id")::text FROM "concerts" WHERE "concerts"."id" = $1
    /// ```
    fn update_transaction_script<'a>(
        &self,
        abstract_update: AbstractUpdate,
        database: &'a Database,
        transformer: &Postgres,
        transaction_script: &mut TransactionScript<'a>,
    ) {
        add_precheck_queries(
            abstract_update.precheck_predicates,
            database,
            transformer,
            transaction_script,
        );

        let predicate = transformer.to_predicate(
            &abstract_update.predicate,
            &SelectionLevel::TopLevel,
            false,
            database,
        );

        // Select only the primary key column, so that we can use that
        // as the proxy column in the nested updates added to the transaction script.
        let return_cols = database
            .get_pk_column_ids(abstract_update.table_id)
            .into_iter()
            .map(|pk_column_id| Column::physical(pk_column_id, None))
            .collect::<Vec<_>>();

        let table = database.get_table(abstract_update.table_id);

        // If there are no self columns to update, just select the primary key columns
        let root_step = if abstract_update.column_values.is_empty() {
            SQLOperation::Select(Select {
                table: Table::Physical {
                    table_id: abstract_update.table_id,
                    alias: None,
                },
                columns: return_cols,
                predicate,
                order_by: None,
                offset: None,
                limit: None,
                group_by: None,
                top_level_selection: false,
            })
        } else {
            let column_id_values: Vec<(ColumnId, MaybeOwned<'a, Column>)> = abstract_update
                .column_values
                .into_iter()
                .map(|(c, v)| (c, v.into()))
                .collect();

            let column_values = column_id_values
                .into_iter()
                .map(|(col_id, col)| (col_id.get_column(database), col))
                .collect();
            SQLOperation::Update(table.update(
                column_values,
                predicate.into(),
                return_cols.into_iter().map(|c| c.into()).collect(),
            ))
        };

        let root_step_id = transaction_script.add_step(TransactionStep::Concrete(Box::new(
            ConcreteTransactionStep::new(root_step),
        )));

        abstract_update
            .nested_updates
            .into_iter()
            .for_each(|nested_update| {
                // Create a template update operation and bind it to the root step
                let update_op = TemplateTransactionStep {
                    operation: update_op(nested_update, transformer, database),
                    prev_step_id: root_step_id,
                };

                let _ = transaction_script.add_step(TransactionStep::Template(update_op));
            });

        abstract_update.nested_inserts.into_iter().for_each(
            |NestedAbstractInsertSet {
                 ops,
                 filter_predicate,
             }| {
                let filter_step_predicate = transformer.to_predicate(
                    &filter_predicate,
                    &SelectionLevel::TopLevel,
                    false,
                    database,
                );

                let filter_step_id =
                    transaction_script.add_step(TransactionStep::Filter(TemplateFilterOperation {
                        table_id: abstract_update.table_id,
                        prev_step_id: root_step_id,
                        predicate: filter_step_predicate,
                    }));

                for nested_insert in ops {
                    add_insert_steps(
                        nested_insert,
                        filter_step_id,
                        database,
                        transformer,
                        transaction_script,
                    );
                }
            },
        );

        abstract_update
            .nested_deletes
            .into_iter()
            .for_each(|nested_delete| {
                let delete_op = TemplateTransactionStep {
                    operation: delete_op(nested_delete, transformer, database),
                    prev_step_id: root_step_id,
                };

                let _ = transaction_script.add_step(TransactionStep::Template(delete_op));
            });

        let select = transformer.to_select(abstract_update.selection, database);

        let table_id = abstract_update.table_id;

        // Take the root step and use ids returned by it as the input to the select
        // statement to form a predicate `pk IN (update_pk1, update_pk2, ...)`
        let select_transformation = Box::new(move |transaction_context: &TransactionContext| {
            let update_count = transaction_context.row_count(root_step_id);

            let predicate = database
                .get_table(table_id)
                .get_pk_column_indices()
                .into_iter()
                .enumerate()
                .fold(
                    select.predicate,
                    |predicate, (pk_column_index, pk_physical_column_index)| {
                        let pk_column_id = ColumnId {
                            table_id,
                            column_index: pk_physical_column_index,
                        };
                        let pk_physical_column =
                            &database.get_table(table_id).columns[pk_physical_column_index];

                        let in_values = SQLParamContainer::from_sql_values(
                            (0..update_count)
                                .map(|row_index| {
                                    transaction_context.resolve_value(
                                        root_step_id,
                                        row_index,
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

fn update_op<'a>(
    nested_update: NestedAbstractUpdate,
    predicate_transformer: &impl PredicateTransformer,
    database: &'a Database,
) -> TemplateSQLOperation<'a> {
    let column_values: Vec<(&PhysicalColumn, Column)> = nested_update
        .update
        .column_values
        .into_iter()
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
        nesting_relation: nested_update.nesting_relation.clone(),
        column_values,
        returning: vec![],
    })
}

fn add_insert_steps<'a>(
    nested_insert: NestedAbstractInsert,
    parent_step_id: TransactionStepId,
    database: &'a Database,
    transformer: &Postgres,
    transaction_script: &mut TransactionScript<'a>,
) {
    let NestedAbstractInsert {
        insert,
        relation_column_ids,
    } = nested_insert;

    transformer.update_transaction_script(
        insert,
        Some((parent_step_id, relation_column_ids.clone())),
        database,
        transaction_script,
    );
}

fn delete_op<'a>(
    nested_delete: NestedAbstractDelete,
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
        nesting_relation: nested_delete.nesting_relation.clone(),
        returning: vec![],
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        PhysicalColumnPath,
        asql::{
            column_path::ColumnPath,
            predicate::AbstractPredicate,
            select::AbstractSelect,
            selection::{AliasedSelectionElement, Selection, SelectionElement},
            update::NestedAbstractUpdate,
        },
        get_otm_relation_for_columns,
        sql::{SQLParamContainer, column::Column, predicate::Predicate},
        transform::{test_util::TestSetup, transformer::UpdateTransformer},
    };

    use multiplatform_test::multiplatform_test;

    use super::*;

    #[multiplatform_test]
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
                let literal = ColumnPath::Param(SQLParamContainer::i32(5));
                let predicate = AbstractPredicate::eq(venue_id_path, literal);

                let abs_update = AbstractUpdate {
                    table_id: venues_table,
                    predicate,
                    column_values: vec![(
                        venues_name_column,
                        Column::Param(SQLParamContainer::string("new_name".to_string())),
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
                    precheck_predicates: vec![],
                };

                let update =
                    UpdateTransformer::to_transaction_script(&Postgres {}, abs_update, &database);

                // TODO: Add a proper assertion here (ideally, we can get a digest of the transaction script and assert on it)
                println!("{update:#?}");
            },
        )
    }

    #[multiplatform_test]
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

                let literal = ColumnPath::Param(SQLParamContainer::i32(5));
                let predicate = AbstractPredicate::eq(venue_id_path, literal);

                let nested_abs_update = NestedAbstractUpdate {
                    nesting_relation: get_otm_relation_for_columns(
                        &[concerts_venue_id_column],
                        &database,
                    )
                    .unwrap()
                    .deref(&database),
                    update: AbstractUpdate {
                        table_id: concerts_table,
                        predicate: Predicate::True,
                        column_values: vec![(
                            concerts_name_column,
                            Column::Param(SQLParamContainer::string(
                                "new_concert_name".to_string(),
                            )),
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
                        precheck_predicates: vec![],
                    },
                };

                let abs_update = AbstractUpdate {
                    table_id: venues_table,
                    predicate,
                    column_values: vec![(
                        venues_name_column,
                        Column::Param(SQLParamContainer::string("new_name".to_string())),
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
                    precheck_predicates: vec![],
                };

                let update =
                    UpdateTransformer::to_transaction_script(&Postgres {}, abs_update, &database);

                // TODO: Add a proper assertion here (ideally, we can get a digest of the transaction script and assert on it)
                println!("{update:#?}");
            },
        )
    }
}
