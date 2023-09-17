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
        column::ArrayParamWrapper,
        delete::TemplateDelete,
        select::Select,
        sql_operation::TemplateSQLOperation,
        transaction::{
            DynamicTransactionStep, TemplateFilterOperation, TemplateTransactionStep,
            TransactionContext, TransactionStepId,
        },
        update::TemplateUpdate,
    },
    transform::transformer::{InsertTransformer, PredicateTransformer},
    ColumnId, NestedAbstractDelete, NestedAbstractInsert, NestedAbstractInsertSet,
    NestedAbstractUpdate, PhysicalColumn, Predicate, SQLParamContainer,
};

use crate::{
    sql::{
        sql_operation::SQLOperation,
        transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep},
    },
    transform::{
        pg::{selection_level::SelectionLevel, Postgres},
        transformer::SelectTransformer,
    },
    AbstractUpdate, Column, Database,
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
        abstract_update: &'a AbstractUpdate,
        database: &'a Database,
        transformer: &Postgres,
        transaction_script: &mut TransactionScript<'a>,
    ) {
        let column_id_values: Vec<(ColumnId, MaybeOwned<'a, Column>)> = abstract_update
            .column_values
            .iter()
            .map(|(c, v)| (*c, v.into()))
            .collect();

        let predicate = transformer.to_predicate(
            &abstract_update.predicate,
            &SelectionLevel::TopLevel,
            false,
            database,
        );

        // Select only the primary key column, so that we can use that
        // as the proxy column in the nested updates added to the transaction script.
        let return_col = Column::physical(
            database
                .get_pk_column_id(abstract_update.table_id)
                .expect("No primary key column"),
            None,
        );

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

        let root_step_id = transaction_script.add_step(TransactionStep::Concrete(
            ConcreteTransactionStep::new(root_update),
        ));

        abstract_update
            .nested_updates
            .iter()
            .for_each(|nested_update| {
                // Create a template update operation and bind it to the root step
                let update_op = TemplateTransactionStep {
                    operation: update_op(nested_update, transformer, database),
                    prev_step_id: root_step_id,
                };

                let _ = transaction_script.add_step(TransactionStep::Template(update_op));
            });

        abstract_update.nested_inserts.iter().for_each(
            |NestedAbstractInsertSet {
                 ops,
                 filter_predicate,
             }| {
                let filter_step_predicate = transformer.to_predicate(
                    filter_predicate,
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
            .iter()
            .for_each(|nested_delete| {
                let delete_op = TemplateTransactionStep {
                    operation: delete_op(nested_delete, transformer, database),
                    prev_step_id: root_step_id,
                };

                let _ = transaction_script.add_step(TransactionStep::Template(delete_op));
            });

        let select = transformer.to_select(&abstract_update.selection, database);

        // Take the root step and use ids returned by it as the input to the select
        // statement to form a predicate `pk IN (update_pk1, update_pk2, ...)`
        let select_transformation = Box::new(move |transaction_context: &TransactionContext| {
            let update_count = transaction_context.row_count(root_step_id);
            let update_ids = SQLParamContainer::new(
                (0..update_count)
                    .map(|i| transaction_context.resolve_value(root_step_id, i, 0))
                    .collect::<Vec<_>>(),
            );

            let predicate = Predicate::and(
                Predicate::Eq(
                    Column::physical(
                        database
                            .get_pk_column_id(abstract_update.table_id)
                            .expect("No primary key column"),
                        None,
                    ),
                    Column::ArrayParam {
                        param: update_ids,
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

fn add_insert_steps<'a>(
    nested_insert: &'a NestedAbstractInsert,
    parent_step_id: TransactionStepId,
    database: &'a Database,
    transformer: &Postgres,
    transaction_script: &mut TransactionScript<'a>,
) {
    let NestedAbstractInsert {
        insert,
        relation_column_id,
    } = nested_insert;

    transformer.update_transaction_script(
        insert,
        Some((parent_step_id, *relation_column_id)),
        database,
        transaction_script,
    );
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
        transform::{test_util::TestSetup, transformer::UpdateTransformer},
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
