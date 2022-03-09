use crate::sql::{
    column::{Column, PhysicalColumn, ProxyColumn},
    predicate::Predicate,
    transaction::{
        ConcreteTransactionStep, TemplateTransactionStep, TransactionScript, TransactionStep,
    },
    Cte, PhysicalTable, SQLOperation, TemplateSQLOperation, TemplateUpdate,
};

use super::{
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
    pub nested_update: Option<Vec<NestedAbstractUpdate<'a>>>,
}

#[derive(Debug)]
pub struct NestedAbstractUpdate<'a> {
    pub relation: NestedElementRelation<'a>,
    pub update: AbstractUpdate<'a>,
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
        let return_col = match &self.nested_update {
            Some(nested_update) if !nested_update.is_empty() => Column::Physical(
                self.table
                    .get_pk_physical_column()
                    .expect("No primary key column"),
            ),
            _ => Column::Star,
        };

        let root_update = SQLOperation::Update(self.table.update(
            column_values,
            predicate.into(),
            vec![return_col.into()],
        ));

        let mut transaction_script = TransactionScript::default();

        match self.nested_update {
            Some(nested_updates) if !nested_updates.is_empty() => {
                let root_step_id = transaction_script.add_step(TransactionStep::Concrete(
                    ConcreteTransactionStep::new(root_update),
                ));
                nested_updates.into_iter().for_each(|nested_update| {
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
                            step_id: root_step_id,
                        },
                    ));

                    let _ = transaction_script.add_step(TransactionStep::Template(
                        TemplateTransactionStep {
                            operation: TemplateSQLOperation::Update(TemplateUpdate {
                                table: nested_update.update.table,
                                predicate: nested_update
                                    .update
                                    .predicate
                                    .map(|p| p.predicate())
                                    .unwrap_or(Predicate::True),
                                column_values,
                                returning: vec![],
                            }),
                            prev_step_id: root_step_id,
                        },
                    ));
                });
            }
            _ => {
                transaction_script.add_step(TransactionStep::Concrete(
                    ConcreteTransactionStep::new(SQLOperation::Cte(Cte {
                        ctes: vec![(self.table.name.clone(), root_update)],
                        select,
                    })),
                ));
            }
        }

        transaction_script
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
                    nested_update: None,
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
                        nested_update: None,
                    },
                };

                let abs_update = AbstractUpdate {
                    table: venues_table,
                    predicate: Some(predicate),
                    column_values: vec![(
                        venues_name_column,
                        Column::Literal(MaybeOwned::Owned(Box::new("new_name".to_string()))),
                    )],
                    nested_update: Some(vec![nested_abs_update]),
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
