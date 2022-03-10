use crate::sql::{
    column::{Column, PhysicalColumn, ProxyColumn},
    predicate::Predicate,
    transaction::{
        ConcreteTransactionStep, TemplateTransactionStep, TransactionScript, TransactionStep,
        TransactionStepId,
    },
    Cte, PhysicalTable, SQLOperation, TemplateDelete, TemplateInsert, TemplateSQLOperation,
    TemplateUpdate,
};

use super::{
    delete::AbstractDelete,
    insert::AbstractInsert,
    predicate::AbstractPredicate,
    select::{AbstractSelect, SelectionLevel},
    selection::NestedElementRelation,
};

/// Abstract representation of an update statement.
///
/// An update may have nested create, update, and delete operations. This supports updating a tree of entities
/// starting at the root table. For example, while updating a concert, this allows adding a new concert-artist,
/// updating (say, role or rank) of an existing concert-artist, or deleting an existing concert-artist.
#[derive(Debug)]
pub struct AbstractUpdate<'a> {
    pub table: &'a PhysicalTable,
    pub predicate: Option<AbstractPredicate<'a>>,
    pub column_values: Vec<(&'a PhysicalColumn, Column<'a>)>,
    pub selection: AbstractSelect<'a>,
    pub nested_updates: Vec<NestedAbstractUpdate<'a>>,
    pub nested_inserts: Vec<NestedAbstractInsert<'a>>,
    pub nested_deletes: Vec<NestedAbstractDelete<'a>>,
}

#[derive(Debug)]
pub struct NestedAbstractUpdate<'a> {
    pub relation: NestedElementRelation<'a>,
    pub update: AbstractUpdate<'a>,
}

#[derive(Debug)]
pub struct NestedAbstractInsert<'a> {
    pub relation: NestedElementRelation<'a>,
    pub insert: AbstractInsert<'a>,
}

#[derive(Debug)]
pub struct NestedAbstractDelete<'a> {
    pub relation: NestedElementRelation<'a>,
    pub delete: AbstractDelete<'a>,
}

impl<'a> AbstractUpdate<'a> {
    pub fn to_sql(self, additional_predicate: Option<Predicate<'a>>) -> TransactionScript<'a> {
        let column_values = self.column_values;

        // TODO: Consider the "join" aspect of the predicate
        let predicate = Predicate::and(
            self.predicate
                .map(|p| p.predicate())
                .unwrap_or_else(|| Predicate::True),
            additional_predicate.unwrap_or(Predicate::True),
        );

        let select = self.selection.to_sql(None, SelectionLevel::TopLevel);

        // If there is no nested update, select all the columns, so that the select statement will have all
        // those column (and not have to specify the WHERE clause once again).
        // If there are nested updates, select only the primary key columns, so that we can use that as the proxy
        // column in the nested updates added to the transaction script.
        let return_col = if !self.nested_updates.is_empty() {
            Column::Physical(
                self.table
                    .get_pk_physical_column()
                    .expect("No primary key column"),
            )
        } else {
            Column::Star
        };

        let root_update = SQLOperation::Update(self.table.update(
            column_values,
            predicate.into(),
            vec![return_col.into()],
        ));

        let mut transaction_script = TransactionScript::default();

        if !self.nested_updates.is_empty()
            || !self.nested_inserts.is_empty()
            || !self.nested_deletes.is_empty()
        {
            let root_step_id = transaction_script.add_step(TransactionStep::Concrete(
                ConcreteTransactionStep::new(root_update),
            ));

            self.nested_updates.into_iter().for_each(|nested_update| {
                let update_op = TemplateTransactionStep {
                    operation: Self::update_op(nested_update, root_step_id),
                    prev_step_id: root_step_id,
                };

                let _ = transaction_script.add_step(TransactionStep::Template(update_op));
            });

            self.nested_inserts.into_iter().for_each(|nested_insert| {
                let insert_op = TemplateTransactionStep {
                    operation: Self::insert_op(nested_insert, root_step_id),
                    prev_step_id: root_step_id,
                };

                let _ = transaction_script.add_step(TransactionStep::Template(insert_op));
            });

            self.nested_deletes.into_iter().for_each(|nested_delete| {
                let delete_op = TemplateTransactionStep {
                    operation: Self::delete_op(nested_delete, root_step_id),
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
                    ctes: vec![(self.table.name.clone(), root_update)],
                    select,
                }),
            )));
        }

        transaction_script
    }

    fn update_op(
        nested_update: NestedAbstractUpdate,
        parent_step_id: TransactionStepId,
    ) -> TemplateSQLOperation {
        let mut column_values: Vec<_> = nested_update
            .update
            .column_values
            .into_iter()
            .map(|(col, val)| (col, ProxyColumn::Concrete(val.into())))
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
            predicate: nested_update
                .update
                .predicate
                .map(|p| p.predicate())
                .unwrap_or(Predicate::True),
            column_values,
            returning: vec![],
        })
    }

    fn insert_op(
        nested_insert: NestedAbstractInsert,
        parent_step_id: TransactionStepId,
    ) -> TemplateSQLOperation {
        let rows = nested_insert.insert.rows;

        // TODO: Deal with _nested_elems (i.e. a recursive use of nested insert)
        let (self_elems, _nested_elems): (Vec<_>, Vec<_>) = rows
            .into_iter()
            .map(|row| row.partition_self_and_nested())
            .unzip();

        let (mut column_names, column_values_seq) = super::insert::align(self_elems);
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

    fn delete_op(
        nested_delete: NestedAbstractDelete,
        _parent_step_id: TransactionStepId,
    ) -> TemplateSQLOperation {
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

        let predicate = nested_delete
            .delete
            .predicate
            .map(|p| p.predicate())
            .unwrap_or_else(|| Predicate::True);

        TemplateSQLOperation::Delete(TemplateDelete {
            table: nested_delete.delete.table,
            predicate,
            returning: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use maybe_owned::MaybeOwned;

    use crate::asql::{
        column_path::{ColumnPath, ColumnPathLink},
        selection::{ColumnSelection, Selection, SelectionElement},
        test_util::TestSetup,
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
                    predicate: Some(predicate),
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
                        predicate: None,
                        order_by: None,
                        offset: None,
                        limit: None,
                    },
                };

                let update = abs_update.to_sql(None);

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
                        predicate: None,
                        column_values: vec![(
                            concerts_name_column,
                            Column::Literal(MaybeOwned::Owned(Box::new(
                                "new_concert_name".to_string(),
                            ))),
                        )],
                        selection: AbstractSelect {
                            selection: Selection::Seq(vec![]),
                            table: venues_table,
                            predicate: None,
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
                    predicate: Some(predicate),
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
                        predicate: None,
                        order_by: None,
                        offset: None,
                        limit: None,
                    },
                };

                let update = abs_update.to_sql(None);

                println!("{:#?}", update);
            },
        )
    }
}
