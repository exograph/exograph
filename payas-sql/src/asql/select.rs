use crate::{
    asql::{
        column_path::{ColumnPath, ColumnPathLink},
        selection::SelectionSQL,
        util,
    },
    sql::{
        predicate::Predicate,
        sql_operation::SQLOperation,
        transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep},
        Limit, Offset, PhysicalTable, Select,
    },
};

use super::{order_by::AbstractOrderBy, predicate::AbstractPredicate, selection::Selection};

#[derive(Debug)]
pub struct AbstractSelect<'a> {
    pub table: &'a PhysicalTable,
    pub selection: Selection<'a>,
    pub predicate: Option<AbstractPredicate<'a>>,
    pub order_by: Option<AbstractOrderBy<'a>>,
    pub offset: Option<Offset>,
    pub limit: Option<Limit>,
}

pub enum SelectionLevel {
    TopLevel,
    Nested,
}

impl<'a> AbstractSelect<'a> {
    pub fn to_select(
        &'a self,
        additional_predicate: Option<Predicate<'a>>,
        selection_level: SelectionLevel,
    ) -> Select<'a> {
        fn column_path_owned<'a>(
            column_paths: Vec<&ColumnPath<'a>>,
        ) -> Vec<Vec<ColumnPathLink<'a>>> {
            column_paths
                .into_iter()
                .filter_map(|path| match path {
                    ColumnPath::Physical(links) => Some(links.to_vec()),
                    _ => None,
                })
                .collect()
        }

        let predicate_column_paths: Vec<Vec<ColumnPathLink>> = self
            .predicate
            .as_ref()
            .map(|predicate| column_path_owned(predicate.column_paths()))
            .unwrap_or_else(Vec::new);

        let order_by_column_paths = self
            .order_by
            .as_ref()
            .map(|ob| column_path_owned(ob.column_paths()))
            .unwrap_or_else(Vec::new);

        let columns_paths = predicate_column_paths
            .into_iter()
            .chain(order_by_column_paths.into_iter())
            .collect();

        let join = util::compute_join(self.table, columns_paths);

        let columns = match self.selection.to_sql() {
            SelectionSQL::Single(elem) => vec![elem.into()],
            SelectionSQL::Seq(elems) => elems.into_iter().map(|elem| elem.into()).collect(),
        };

        let predicate = Predicate::and(
            self.predicate
                .as_ref()
                .map(|p| p.predicate())
                .unwrap_or_else(|| Predicate::True),
            additional_predicate.unwrap_or(Predicate::True),
        )
        .into();

        Select {
            underlying: join,
            columns,
            predicate,
            order_by: self.order_by.as_ref().map(|ob| ob.order_by()),
            offset: self.offset.clone(),
            limit: self.limit.clone(),
            top_level_selection: matches!(selection_level, SelectionLevel::TopLevel),
        }
    }

    pub(crate) fn to_transaction_script(
        &'a self,
        additional_predicate: Option<Predicate<'a>>,
    ) -> TransactionScript<'a> {
        let select = self.to_select(additional_predicate, SelectionLevel::TopLevel);
        let mut transaction_script = TransactionScript::default();
        transaction_script.add_step(TransactionStep::Concrete(ConcreteTransactionStep::new(
            SQLOperation::Select(select),
        )));
        transaction_script
    }
}

#[cfg(test)]
mod tests {
    use maybe_owned::MaybeOwned;

    use crate::{
        asql::{
            column_path::{ColumnPath, ColumnPathLink},
            predicate::AbstractPredicate,
            select::SelectionLevel,
            selection::{
                ColumnSelection, NestedElementRelation, Selection, SelectionCardinality,
                SelectionElement,
            },
            test_util::TestSetup,
        },
        sql::ExpressionContext,
    };

    use super::AbstractSelect;
    use crate::sql::Expression;

    #[test]
    fn simple_selection() {
        TestSetup::with_setup(
            |TestSetup {
                 concerts_table,
                 concerts_id_column,
                 ..
             }| {
                let aselect = AbstractSelect {
                    table: concerts_table,
                    selection: Selection::Seq(vec![ColumnSelection::new(
                        "id".to_string(),
                        SelectionElement::Physical(concerts_id_column),
                    )]),
                    predicate: None,
                    order_by: None,
                    offset: None,
                    limit: None,
                };

                let select = aselect.to_select(None, SelectionLevel::TopLevel);
                let mut expr = ExpressionContext::default();
                let binding = select.binding(&mut expr);
                assert_binding!(binding, r#"select "concerts"."id" from "concerts""#);
            },
        );
    }

    #[test]
    fn simple_predicate() {
        TestSetup::with_setup(
            |TestSetup {
                 concerts_table,
                 concerts_id_column,
                 ..
             }| {
                let concert_id_path = ColumnPath::Physical(vec![ColumnPathLink {
                    self_column: (concerts_id_column, concerts_table),
                    linked_column: None,
                }]);
                let literal = ColumnPath::Literal(MaybeOwned::Owned(Box::new(5)));
                let predicate = AbstractPredicate::Eq(concert_id_path.into(), literal.into());

                let aselect = AbstractSelect {
                    table: concerts_table,
                    selection: Selection::Seq(vec![ColumnSelection::new(
                        "id".to_string(),
                        SelectionElement::Physical(concerts_id_column),
                    )]),
                    predicate: Some(predicate),
                    order_by: None,
                    offset: None,
                    limit: None,
                };

                let select = aselect.to_select(None, SelectionLevel::TopLevel);
                let mut expr = ExpressionContext::default();
                let binding = select.binding(&mut expr);
                assert_binding!(
                    binding,
                    r#"select "concerts"."id" from "concerts" WHERE "concerts"."id" = $1"#,
                    5
                );
            },
        );
    }

    #[test]
    fn non_nested_json() {
        TestSetup::with_setup(
            |TestSetup {
                 concerts_table,
                 concerts_id_column,
                 ..
             }| {
                let aselect = AbstractSelect {
                    table: concerts_table,
                    selection: Selection::Json(
                        vec![ColumnSelection::new(
                            "id".to_string(),
                            SelectionElement::Physical(concerts_id_column),
                        )],
                        SelectionCardinality::Many,
                    ),
                    predicate: None,
                    order_by: None,
                    offset: None,
                    limit: None,
                };

                let select = aselect.to_select(None, SelectionLevel::TopLevel);
                let mut expr = ExpressionContext::default();
                let binding = select.binding(&mut expr);
                assert_binding!(
                    binding,
                    r#"select coalesce(json_agg(json_build_object('id', "concerts"."id")), '[]'::json)::text from "concerts""#
                );
            },
        );
    }

    #[test]
    fn nested_many_to_one_json() {
        // {
        //     id: 5,
        //     venue: { // concert.venue_id = venue.id
        //         id: 8
        //     }
        // }
        TestSetup::with_setup(
            |TestSetup {
                 concerts_table,
                 venues_table,
                 concerts_id_column,
                 venues_id_column,
                 concerts_venue_id_column,
                 ..
             }| {
                let aselect = AbstractSelect {
                    table: concerts_table,
                    selection: Selection::Json(
                        vec![
                            ColumnSelection::new(
                                "id".to_string(),
                                SelectionElement::Physical(concerts_id_column),
                            ),
                            ColumnSelection::new(
                                "venue".to_string(),
                                SelectionElement::Nested(
                                    NestedElementRelation::new(
                                        concerts_venue_id_column,
                                        venues_table,
                                    ),
                                    AbstractSelect {
                                        table: venues_table,
                                        selection: Selection::Json(
                                            vec![ColumnSelection::new(
                                                "id".to_string(),
                                                SelectionElement::Physical(venues_id_column),
                                            )],
                                            SelectionCardinality::One,
                                        ),
                                        predicate: None,
                                        order_by: None,
                                        offset: None,
                                        limit: None,
                                    },
                                ),
                            ),
                        ],
                        SelectionCardinality::Many,
                    ),
                    predicate: None,
                    order_by: None,
                    offset: None,
                    limit: None,
                };

                let select = aselect.to_select(None, SelectionLevel::TopLevel);
                let mut expr = ExpressionContext::default();
                let binding = select.binding(&mut expr);
                assert_binding!(
                    binding,
                    r#"select coalesce(json_agg(json_build_object('id', "concerts"."id", 'venue', (select json_build_object('id', "venues"."id") from "venues" WHERE "concerts"."venue_id" = "venues"."id"))), '[]'::json)::text from "concerts""#
                );
            },
        );
    }

    #[test]
    fn nested_one_to_many_json() {
        TestSetup::with_setup(
            |TestSetup {
                 concerts_table,
                 venues_table,
                 concerts_id_column,
                 venues_id_column,
                 concerts_venue_id_column,
                 ..
             }| {
                let aselect = AbstractSelect {
                    table: venues_table,
                    selection: Selection::Json(
                        vec![
                            ColumnSelection::new(
                                "id".to_string(),
                                SelectionElement::Physical(venues_id_column),
                            ),
                            ColumnSelection::new(
                                "concerts".to_string(),
                                SelectionElement::Nested(
                                    NestedElementRelation::new(
                                        concerts_venue_id_column,
                                        venues_table,
                                    ),
                                    AbstractSelect {
                                        table: concerts_table,
                                        selection: Selection::Json(
                                            vec![ColumnSelection::new(
                                                "id".to_string(),
                                                SelectionElement::Physical(concerts_id_column),
                                            )],
                                            SelectionCardinality::Many,
                                        ),
                                        predicate: None,
                                        order_by: None,
                                        offset: None,
                                        limit: None,
                                    },
                                ),
                            ),
                        ],
                        SelectionCardinality::Many,
                    ),
                    predicate: None,
                    order_by: None,
                    offset: None,
                    limit: None,
                };

                let select = aselect.to_select(None, SelectionLevel::TopLevel);
                let mut expr = ExpressionContext::default();
                let binding = select.binding(&mut expr);
                assert_binding!(
                    binding,
                    r#"select coalesce(json_agg(json_build_object('id', "venues"."id", 'concerts', (select coalesce(json_agg(json_build_object('id', "concerts"."id")), '[]'::json) from "concerts" WHERE "concerts"."venue_id" = "venues"."id"))), '[]'::json)::text from "venues""#
                );
            },
        );
    }

    #[test]
    fn nested_predicate() {
        TestSetup::with_setup(
            |TestSetup {
                 concerts_table,
                 concerts_id_column,
                 concerts_venue_id_column,
                 venues_table,
                 venues_id_column,
                 venues_name_column,
                 ..
             }| {
                // {
                //     concerts(where: {venue: {name: {eq: "v1"}}}) {
                //       id
                //     }
                // }
                let predicate = AbstractPredicate::Eq(
                    ColumnPath::Physical(vec![
                        ColumnPathLink {
                            self_column: (concerts_venue_id_column, concerts_table),
                            linked_column: Some((venues_id_column, venues_table)),
                        },
                        ColumnPathLink {
                            self_column: (venues_name_column, venues_table),
                            linked_column: None,
                        },
                    ])
                    .into(),
                    ColumnPath::Literal(MaybeOwned::Owned(Box::new("v1".to_string()))).into(),
                );
                let aselect = AbstractSelect {
                    table: concerts_table,
                    selection: Selection::Json(
                        vec![ColumnSelection::new(
                            "id".to_string(),
                            SelectionElement::Physical(concerts_id_column),
                        )],
                        SelectionCardinality::Many,
                    ),
                    predicate: Some(predicate),
                    order_by: None,
                    offset: None,
                    limit: None,
                };

                let select = aselect.to_select(None, SelectionLevel::TopLevel);
                let mut expr = ExpressionContext::default();
                let binding = select.binding(&mut expr);
                assert_binding!(
                    binding,
                    r#"select coalesce(json_agg(json_build_object('id', "concerts"."id")), '[]'::json)::text from "concerts" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id" WHERE "venues"."name" = $1"#,
                    "v1".to_string()
                );
            },
        );
    }
}
