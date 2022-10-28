use maybe_owned::MaybeOwned;
use tracing::instrument;

use crate::{
    asql::{
        select::SelectionLevel,
        update::{
            AbstractUpdate, NestedAbstractDelete, NestedAbstractInsert, NestedAbstractUpdate,
        },
    },
    sql::{
        column::{Column, PhysicalColumn, ProxyColumn},
        cte::Cte,
        delete::TemplateDelete,
        insert::TemplateInsert,
        predicate::Predicate,
        sql_operation::{SQLOperation, TemplateSQLOperation},
        transaction::{
            ConcreteTransactionStep, TemplateTransactionStep, TransactionScript, TransactionStep,
            TransactionStepId,
        },
        update::TemplateUpdate,
    },
    transform::transformer::{SelectTransformer, UpdateTransformer},
};

use super::Postgres;

impl UpdateTransformer for Postgres {
    #[instrument(
        name = "UpdateTransformer::to_transaction_script for Postgres"
        skip(self)
        )]
    fn to_transaction_script<'a>(
        &self,
        abstract_select: &'a AbstractUpdate,
        additional_predicate: Option<Predicate<'a>>,
    ) -> TransactionScript<'a> {
        let column_values: Vec<(&'a PhysicalColumn, MaybeOwned<'a, Column<'a>>)> = abstract_select
            .column_values
            .iter()
            .map(|(c, v)| (*c, v.into()))
            .collect();

        // TODO: Consider the "join" aspect of the predicate
        let predicate = Predicate::and(
            abstract_select.predicate.predicate(),
            additional_predicate.unwrap_or(Predicate::True),
        );

        let select = self.to_select(&abstract_select.selection, None, SelectionLevel::TopLevel);

        // If there is no nested update, select all the columns, so that the select statement will have all
        // those column (and not have to specify the WHERE clause once again).
        // If there are nested updates, select only the primary key columns, so that we can use that as the proxy
        // column in the nested updates added to the transaction script.
        let return_col = if !abstract_select.nested_updates.is_empty() {
            Column::Physical(
                abstract_select
                    .table
                    .get_pk_physical_column()
                    .expect("No primary key column"),
            )
        } else {
            Column::Star
        };

        let root_update = SQLOperation::Update(abstract_select.table.update(
            column_values,
            predicate.into(),
            vec![return_col.into()],
        ));

        let mut transaction_script = TransactionScript::default();

        if !abstract_select.nested_updates.is_empty()
            || !abstract_select.nested_inserts.is_empty()
            || !abstract_select.nested_deletes.is_empty()
        {
            let root_step_id = transaction_script.add_step(TransactionStep::Concrete(
                ConcreteTransactionStep::new(root_update),
            ));

            abstract_select
                .nested_updates
                .iter()
                .for_each(|nested_update| {
                    let update_op = TemplateTransactionStep {
                        operation: update_op(nested_update, root_step_id),
                        prev_step_id: root_step_id,
                    };

                    let _ = transaction_script.add_step(TransactionStep::Template(update_op));
                });

            abstract_select
                .nested_inserts
                .iter()
                .for_each(|nested_insert| {
                    let insert_op = TemplateTransactionStep {
                        operation: insert_op(nested_insert, root_step_id),
                        prev_step_id: root_step_id,
                    };

                    let _ = transaction_script.add_step(TransactionStep::Template(insert_op));
                });

            abstract_select
                .nested_deletes
                .iter()
                .for_each(|nested_delete| {
                    let delete_op = TemplateTransactionStep {
                        operation: delete_op(nested_delete, root_step_id),
                        prev_step_id: root_step_id,
                    };

                    let _ = transaction_script.add_step(TransactionStep::Template(delete_op));
                });

            let _ = transaction_script.add_step(TransactionStep::Concrete(
                ConcreteTransactionStep::new(SQLOperation::Select(select)),
            ));
        } else {
            transaction_script.add_step(TransactionStep::Concrete(ConcreteTransactionStep::new(
                SQLOperation::Cte(Cte {
                    ctes: vec![(abstract_select.table.name.clone(), root_update)],
                    select,
                }),
            )));
        }

        transaction_script
    }
}

fn update_op<'a>(
    nested_update: &'a NestedAbstractUpdate,
    parent_step_id: TransactionStepId,
) -> TemplateSQLOperation<'a> {
    let mut column_values: Vec<(&'a PhysicalColumn, ProxyColumn<'a>)> = nested_update
        .update
        .column_values
        .iter()
        .map(|(col, val)| (*col, ProxyColumn::Concrete(val.into())))
        .collect();
    column_values.push((
        nested_update.relation.column,
        ProxyColumn::Template {
            col_index: 0,
            step_id: parent_step_id,
        },
    ));

    TemplateSQLOperation::Update(TemplateUpdate {
        table: nested_update.update.table,
        predicate: nested_update.update.predicate.predicate(),
        column_values,
        returning: vec![],
    })
}

fn insert_op<'a>(
    nested_insert: &'a NestedAbstractInsert,
    parent_step_id: TransactionStepId,
) -> TemplateSQLOperation<'a> {
    let rows = &nested_insert.insert.rows;

    // TODO: Deal with _nested_elems (i.e. a recursive use of nested insert)
    let (self_elems, _nested_elems): (Vec<_>, Vec<_>) = rows
        .iter()
        .map(|row| row.partition_self_and_nested())
        .unzip();

    let (mut column_names, column_values_seq) = super::insert_transformer::align(self_elems);
    column_names.push(nested_insert.relation.column);

    let column_values_seq: Vec<_> = column_values_seq
        .into_iter()
        .map(|column_values| {
            let mut column_values: Vec<_> = column_values
                .into_iter()
                .map(ProxyColumn::Concrete)
                .collect();
            column_values.push(ProxyColumn::Template {
                col_index: 0,
                step_id: parent_step_id,
            });
            column_values
        })
        .collect();

    TemplateSQLOperation::Insert(TemplateInsert {
        table: nested_insert.insert.table,
        column_names,
        column_values_seq,
        returning: vec![],
    })
}

fn delete_op<'a>(
    nested_delete: &'a NestedAbstractDelete,
    _parent_step_id: TransactionStepId,
) -> TemplateSQLOperation<'a> {
    // TODO: We need TemplatePredicate here, because we need to use the proxy column in the nested delete
    // let predicate = Predicate::and(
    //     nested_delete
    //         .delete
    //         .predicate
    //         .map(|p| p.predicate())
    //         .unwrap_or_else(|| Predicate::True),
    //     Predicate::Eq(
    //         Box::new(nested_delete.relation.column.into()),
    //         Box::new(ProxyColumn::Template {
    //             col_index: 0,
    //             step_id: parent_step_id,
    //         }),
    //     ),
    // );

    let predicate = nested_delete.delete.predicate.predicate();

    TemplateSQLOperation::Delete(TemplateDelete {
        table: nested_delete.delete.table,
        predicate,
        returning: vec![],
    })
}

#[cfg(test)]
mod tests {
    use maybe_owned::MaybeOwned;

    use crate::{
        asql::{
            column_path::{ColumnPath, ColumnPathLink},
            predicate::AbstractPredicate,
            select::AbstractSelect,
            selection::{ColumnSelection, NestedElementRelation, Selection, SelectionElement},
            update::NestedAbstractUpdate,
        },
        sql::column::Column,
        transform::test_util::TestSetup,
    };

    use super::*;

    #[test]
    fn simple_update() {
        TestSetup::with_setup(
            |TestSetup {
                 venues_table,
                 venues_id_column,
                 venues_name_column,
                 ..
             }| {
                let venue_id_path = ColumnPath::Physical(vec![ColumnPathLink {
                    self_column: (venues_id_column, venues_table),
                    linked_column: None,
                }]);
                let literal = ColumnPath::Literal(MaybeOwned::Owned(Box::new(5)));
                let predicate = AbstractPredicate::eq(venue_id_path.into(), literal.into());

                let abs_update = AbstractUpdate {
                    table: venues_table,
                    predicate,
                    column_values: vec![(
                        venues_name_column,
                        Column::Literal(MaybeOwned::Owned(Box::new("new_name".to_string()))),
                    )],
                    nested_updates: vec![],
                    nested_inserts: vec![],
                    nested_deletes: vec![],
                    selection: AbstractSelect {
                        selection: Selection::Seq(vec![
                            ColumnSelection::new(
                                "id".to_string(),
                                SelectionElement::Physical(venues_id_column),
                            ),
                            ColumnSelection::new(
                                "name".to_string(),
                                SelectionElement::Physical(venues_name_column),
                            ),
                        ]),
                        table: venues_table,
                        predicate: Predicate::True,
                        order_by: None,
                        offset: None,
                        limit: None,
                    },
                };

                let update =
                    UpdateTransformer::to_transaction_script(&Postgres {}, &abs_update, None);

                // TODO: Add a proper assertion here (ideally, we can get a digest of the transaction script and assert on it)
                println!("{:#?}", update);
            },
        )
    }

    #[test]
    fn nested_update() {
        TestSetup::with_setup(
            |TestSetup {
                 venues_table,
                 venues_id_column,
                 venues_name_column,
                 concerts_table,
                 concerts_name_column,
                 concerts_venue_id_column,
                 ..
             }| {
                let venue_id_path = ColumnPath::Physical(vec![ColumnPathLink {
                    self_column: (venues_id_column, venues_table),
                    linked_column: None,
                }]);
                let literal = ColumnPath::Literal(MaybeOwned::Owned(Box::new(5)));
                let predicate = AbstractPredicate::eq(venue_id_path.into(), literal.into());

                let nested_abs_update = NestedAbstractUpdate {
                    relation: NestedElementRelation {
                        column: concerts_venue_id_column,
                        table: concerts_table,
                    },
                    update: AbstractUpdate {
                        table: concerts_table,
                        predicate: Predicate::True,
                        column_values: vec![(
                            concerts_name_column,
                            Column::Literal(MaybeOwned::Owned(Box::new(
                                "new_concert_name".to_string(),
                            ))),
                        )],
                        selection: AbstractSelect {
                            selection: Selection::Seq(vec![]),
                            table: venues_table,
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
                    table: venues_table,
                    predicate,
                    column_values: vec![(
                        venues_name_column,
                        Column::Literal(MaybeOwned::Owned(Box::new("new_name".to_string()))),
                    )],
                    nested_updates: vec![nested_abs_update],
                    nested_inserts: vec![],
                    nested_deletes: vec![],
                    selection: AbstractSelect {
                        selection: Selection::Seq(vec![
                            ColumnSelection::new(
                                "id".to_string(),
                                SelectionElement::Physical(venues_id_column),
                            ),
                            ColumnSelection::new(
                                "name".to_string(),
                                SelectionElement::Physical(venues_name_column),
                            ),
                        ]),
                        table: venues_table,
                        predicate: Predicate::True,
                        order_by: None,
                        offset: None,
                        limit: None,
                    },
                };

                let update =
                    UpdateTransformer::to_transaction_script(&Postgres {}, &abs_update, None);

                // TODO: Add a proper assertion here (ideally, we can get a digest of the transaction script and assert on it)
                println!("{:#?}", update);
            },
        )
    }
}
