use tracing::instrument;

use crate::{
    asql::{
        column_path::{ColumnPath, ColumnPathLink},
        select::{AbstractSelect, SelectionLevel},
        selection::{
            ColumnSelection, Selection, SelectionCardinality, SelectionElement, SelectionSQL,
        },
    },
    sql::{
        column::Column,
        predicate::Predicate,
        select::Select,
        sql_operation::SQLOperation,
        transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep},
    },
    transform::{join_util, transformer::SelectTransformer},
};

use super::Postgres;

impl SelectTransformer for Postgres {
    #[instrument(
        name = "SelectTransformer::to_select for Postgres"
        skip(self)
        )]
    fn to_select<'a>(
        &self,
        abstract_select: &'a AbstractSelect,
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

        let predicate_column_paths: Vec<Vec<ColumnPathLink>> =
            column_path_owned(abstract_select.predicate.column_paths());

        let order_by_column_paths = abstract_select
            .order_by
            .as_ref()
            .map(|ob| column_path_owned(ob.column_paths()))
            .unwrap_or_else(Vec::new);

        let columns_paths = predicate_column_paths
            .into_iter()
            .chain(order_by_column_paths.into_iter())
            .collect();

        let join = join_util::compute_join(abstract_select.table, columns_paths);

        let columns = match abstract_select.selection.to_sql(self) {
            SelectionSQL::Single(elem) => vec![elem.into()],
            SelectionSQL::Seq(elems) => elems.into_iter().map(|elem| elem.into()).collect(),
        };

        let predicate = Predicate::and(
            abstract_select.predicate.predicate(),
            additional_predicate.unwrap_or(Predicate::True),
        )
        .into();

        Select {
            underlying: join,
            columns,
            predicate,
            order_by: abstract_select.order_by.as_ref().map(|ob| ob.order_by()),
            offset: abstract_select.offset.clone(),
            limit: abstract_select.limit.clone(),
            top_level_selection: matches!(selection_level, SelectionLevel::TopLevel),
        }
    }

    fn to_transaction_script<'a>(
        &self,
        abstract_select: &'a AbstractSelect,
        additional_predicate: Option<Predicate<'a>>,
    ) -> TransactionScript<'a> {
        let select = self.to_select(
            abstract_select,
            additional_predicate,
            SelectionLevel::TopLevel,
        );
        let mut transaction_script = TransactionScript::default();
        transaction_script.add_step(TransactionStep::Concrete(ConcreteTransactionStep::new(
            SQLOperation::Select(select),
        )));
        transaction_script
    }
}

impl<'a> Selection<'a> {
    pub fn to_sql(&'a self, database_kind: &impl SelectTransformer) -> SelectionSQL<'a> {
        match self {
            Selection::Seq(seq) => SelectionSQL::Seq(
                seq.iter()
                    .map(
                        |ColumnSelection {
                             alias: _alias,
                             column,
                         }| column.to_sql(database_kind),
                    )
                    .collect(),
            ),
            Selection::Json(seq, cardinality) => {
                let object_elems = seq
                    .iter()
                    .map(|ColumnSelection { alias, column }| {
                        (alias.clone(), column.to_sql(database_kind).into())
                    })
                    .collect();

                let json_obj = Column::JsonObject(object_elems);

                match cardinality {
                    SelectionCardinality::One => SelectionSQL::Single(json_obj),
                    SelectionCardinality::Many => {
                        SelectionSQL::Single(Column::JsonAgg(Box::new(json_obj.into())))
                    }
                }
            }
        }
    }
}

impl<'a> SelectionElement<'a> {
    pub fn to_sql(&'a self, database_kind: &impl SelectTransformer) -> Column<'a> {
        match self {
            SelectionElement::Physical(pc) => Column::Physical(pc),
            SelectionElement::Function {
                function_name,
                column,
            } => Column::Function {
                function_name: function_name.clone(),
                column,
            },
            SelectionElement::Constant(s) => Column::Constant(s.clone()),
            SelectionElement::Object(elements) => {
                let elements = elements
                    .iter()
                    .map(|(alias, column)| (alias.to_owned(), column.to_sql(database_kind).into()))
                    .collect();
                Column::JsonObject(elements)
            }
            SelectionElement::Nested(relation, select) => {
                Column::SelectionTableWrapper(Box::new(database_kind.to_select(
                    select,
                    relation.linked_column.map(|linked_column| {
                        Predicate::Eq(
                            Column::Physical(relation.self_column.0).into(),
                            Column::Physical(linked_column.0).into(),
                        )
                    }),
                    SelectionLevel::Nested,
                )))
            }
        }
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
            selection::{ColumnSelection, Selection, SelectionCardinality, SelectionElement},
        },
        sql::ExpressionContext,
        transform::{pg::Postgres, test_util::TestSetup, transformer::SelectTransformer},
        AbstractOrderBy, Ordering, Predicate,
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
                    predicate: Predicate::True,
                    order_by: None,
                    offset: None,
                    limit: None,
                };

                let select = Postgres {}.to_select(&aselect, None, SelectionLevel::TopLevel);
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
                    predicate,
                    order_by: None,
                    offset: None,
                    limit: None,
                };

                let select = Postgres {}.to_select(&aselect, None, SelectionLevel::TopLevel);
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
                    predicate: Predicate::True,
                    order_by: None,
                    offset: None,
                    limit: None,
                };

                let select = Postgres {}.to_select(&aselect, None, SelectionLevel::TopLevel);
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
                                    ColumnPathLink {
                                        self_column: (concerts_venue_id_column, concerts_table),
                                        linked_column: Some((venues_id_column, venues_table)),
                                    },
                                    AbstractSelect {
                                        table: venues_table,
                                        selection: Selection::Json(
                                            vec![ColumnSelection::new(
                                                "id".to_string(),
                                                SelectionElement::Physical(venues_id_column),
                                            )],
                                            SelectionCardinality::One,
                                        ),
                                        predicate: Predicate::True,
                                        order_by: None,
                                        offset: None,
                                        limit: None,
                                    },
                                ),
                            ),
                        ],
                        SelectionCardinality::Many,
                    ),
                    predicate: Predicate::True,
                    order_by: None,
                    offset: None,
                    limit: None,
                };

                let select = Postgres {}.to_select(&aselect, None, SelectionLevel::TopLevel);
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
                                    ColumnPathLink {
                                        self_column: (concerts_venue_id_column, concerts_table),
                                        linked_column: Some((venues_id_column, venues_table)),
                                    },
                                    AbstractSelect {
                                        table: concerts_table,
                                        selection: Selection::Json(
                                            vec![ColumnSelection::new(
                                                "id".to_string(),
                                                SelectionElement::Physical(concerts_id_column),
                                            )],
                                            SelectionCardinality::Many,
                                        ),
                                        predicate: Predicate::True,
                                        order_by: None,
                                        offset: None,
                                        limit: None,
                                    },
                                ),
                            ),
                        ],
                        SelectionCardinality::Many,
                    ),
                    predicate: Predicate::True,
                    order_by: None,
                    offset: None,
                    limit: None,
                };

                let select = Postgres {}.to_select(&aselect, None, SelectionLevel::TopLevel);
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
                    predicate,
                    order_by: None,
                    offset: None,
                    limit: None,
                };

                let select = Postgres {}.to_select(&aselect, None, SelectionLevel::TopLevel);
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

    #[test]
    fn simple_order_by() {
        TestSetup::with_setup(
            |TestSetup {
                 concerts_table,
                 concerts_id_column,
                 concerts_name_column,
                 ..
             }| {
                let concert_name_path = ColumnPath::Physical(vec![ColumnPathLink {
                    self_column: (concerts_name_column, concerts_table),
                    linked_column: None,
                }]);

                let aselect = AbstractSelect {
                    table: concerts_table,
                    selection: Selection::Seq(vec![ColumnSelection::new(
                        "id".to_string(),
                        SelectionElement::Physical(concerts_id_column),
                    )]),
                    predicate: Predicate::True,
                    order_by: Some(AbstractOrderBy(vec![(concert_name_path, Ordering::Asc)])),
                    offset: None,
                    limit: None,
                };

                let select = Postgres {}.to_select(&aselect, None, SelectionLevel::TopLevel);
                let mut expr = ExpressionContext::default();
                let binding = select.binding(&mut expr);
                assert_binding!(
                    binding,
                    r#"select "concerts"."id" from (select "concerts".* from "concerts" ORDER BY "concerts"."name" ASC) as "concerts""#
                );
            },
        );
    }

    #[test]
    fn nested_order_by() {
        TestSetup::with_setup(
            |TestSetup {
                 concerts_table,
                 venues_table,
                 concerts_id_column,
                 venues_name_column,
                 concerts_venue_id_column,
                 venues_id_column,
                 ..
             }| {
                let venues_name_path = ColumnPath::Physical(vec![
                    ColumnPathLink {
                        self_column: (concerts_venue_id_column, concerts_table),
                        linked_column: Some((venues_id_column, venues_table)),
                    },
                    ColumnPathLink {
                        self_column: (venues_name_column, venues_table),
                        linked_column: None,
                    },
                ]);

                let aselect = AbstractSelect {
                    table: concerts_table,
                    selection: Selection::Seq(vec![ColumnSelection::new(
                        "id".to_string(),
                        SelectionElement::Physical(concerts_id_column),
                    )]),
                    predicate: Predicate::True,
                    order_by: Some(AbstractOrderBy(vec![(venues_name_path, Ordering::Asc)])),
                    offset: None,
                    limit: None,
                };

                let select = Postgres {}.to_select(&aselect, None, SelectionLevel::TopLevel);
                let mut expr = ExpressionContext::default();
                let binding = select.binding(&mut expr);
                assert_binding!(
                    binding,
                    r#"select "concerts"."id" from (select "concerts".* from "concerts" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id" ORDER BY "venues"."name" ASC) as "concerts" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id""#
                );
            },
        );
    }
}
