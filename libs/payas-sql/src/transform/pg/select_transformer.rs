use tracing::instrument;

use crate::{
    asql::{
        column_path::{ColumnPath, ColumnPathLink},
        select::AbstractSelect,
        selection::{
            ColumnSelection, Selection, SelectionCardinality, SelectionElement, SelectionSQL,
        },
    },
    sql::{
        column::Column,
        group_by::GroupBy,
        json_agg::JsonAgg,
        json_object::{JsonObject, JsonObjectElement},
        predicate::ConcretePredicate,
        select::Select,
        sql_operation::SQLOperation,
        table::Table,
        transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep},
    },
    transform::{
        join_util,
        transformer::{OrderByTransformer, PredicateTransformer, SelectTransformer},
        SelectionLevel,
    },
};

use super::Postgres;

impl SelectTransformer for Postgres {
    #[instrument(
        name = "SelectTransformer::to_select for Postgres"
        skip(self)
        )]
    fn to_select<'a>(
        &self,
        abstract_select: &AbstractSelect<'a>,
        additional_predicate: Option<ConcretePredicate<'a>>,
        group_by: Option<GroupBy<'a>>,
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

        let columns_paths: Vec<Vec<ColumnPathLink>> = predicate_column_paths
            .into_iter()
            .chain(order_by_column_paths.into_iter())
            .collect();

        let has_a_many_to_one_clause = columns_paths.iter().any(|path| {
            path.iter()
                .any(|link| link.self_column.0.is_pk && link.linked_column.is_some())
        });

        let join = join_util::compute_join(abstract_select.table, columns_paths);

        let columns = match abstract_select.selection.to_sql(self) {
            SelectionSQL::Single(elem) => vec![elem],
            SelectionSQL::Seq(elems) => elems,
        };

        let has_non_predicate_clauses = abstract_select.order_by.is_some()
            || abstract_select.offset.is_some()
            || abstract_select.limit.is_some();

        if has_a_many_to_one_clause || has_non_predicate_clauses {
            let inner_select = {
                let (predicate, selection_table_query) = if has_a_many_to_one_clause {
                    // If we have a many to one clause, we need to use a subselect along with
                    // the basic table (not a join) to avoid returning duplicate rows (that would be returned by the join)
                    (
                        self.to_subselect_predicate(&abstract_select.predicate),
                        Table::Physical(abstract_select.table),
                    )
                } else {
                    (self.to_join_predicate(&abstract_select.predicate), join)
                };

                let predicate = ConcretePredicate::and(
                    predicate,
                    additional_predicate.unwrap_or(ConcretePredicate::True),
                );

                // Inner select gives the data matching the predicate, order by, offset, limit
                Select {
                    table: selection_table_query,
                    columns: vec![Column::Star(Some(abstract_select.table.name.clone()))],
                    predicate,
                    order_by: abstract_select
                        .order_by
                        .as_ref()
                        .map(|ob| self.to_order_by(ob)),
                    offset: abstract_select.offset.clone(),
                    limit: abstract_select.limit.clone(),
                    group_by,
                    top_level_selection: matches!(selection_level, SelectionLevel::TopLevel),
                }
            };

            // We then use the inner select to build the final select
            Select {
                table: Table::SubSelect {
                    select: Box::new(inner_select),
                    alias: abstract_select.table.name.clone(),
                },
                columns,
                predicate: ConcretePredicate::True,
                order_by: None,
                offset: None,
                limit: None,
                group_by: None,
                top_level_selection: matches!(selection_level, SelectionLevel::TopLevel),
            }
        } else {
            let predicate = ConcretePredicate::and(
                self.to_join_predicate(&abstract_select.predicate),
                additional_predicate.unwrap_or(ConcretePredicate::True),
            );

            Select {
                table: join,
                columns,
                predicate,
                order_by: abstract_select
                    .order_by
                    .as_ref()
                    .map(|ob| self.to_order_by(ob)),
                offset: abstract_select.offset.clone(),
                limit: abstract_select.limit.clone(),
                group_by,
                top_level_selection: matches!(selection_level, SelectionLevel::TopLevel),
            }
        }
    }

    fn to_transaction_script<'a>(
        &self,
        abstract_select: &'a AbstractSelect,
    ) -> TransactionScript<'a> {
        let select = self.to_select(abstract_select, None, None, SelectionLevel::TopLevel);
        let mut transaction_script = TransactionScript::default();
        transaction_script.add_step(TransactionStep::Concrete(ConcreteTransactionStep::new(
            SQLOperation::Select(select),
        )));
        transaction_script
    }
}

impl<'a> Selection<'a> {
    pub fn to_sql(&self, database_kind: &impl SelectTransformer) -> SelectionSQL<'a> {
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
                        JsonObjectElement::new(alias.clone(), column.to_sql(database_kind))
                    })
                    .collect();

                let json_obj = Column::JsonObject(JsonObject(object_elems));

                match cardinality {
                    SelectionCardinality::One => SelectionSQL::Single(json_obj),
                    SelectionCardinality::Many => {
                        SelectionSQL::Single(Column::JsonAgg(JsonAgg(Box::new(json_obj))))
                    }
                }
            }
        }
    }
}

impl<'a> SelectionElement<'a> {
    pub fn to_sql(&self, database_kind: &impl SelectTransformer) -> Column<'a> {
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
                    .map(|(alias, column)| {
                        JsonObjectElement::new(alias.to_owned(), column.to_sql(database_kind))
                    })
                    .collect();
                Column::JsonObject(JsonObject(elements))
            }
            SelectionElement::Nested(relation, select) => {
                Column::SubSelect(Box::new(database_kind.to_select(
                    select,
                    relation.linked_column.map(|linked_column| {
                        ConcretePredicate::Eq(
                            Column::Physical(relation.self_column.0),
                            Column::Physical(linked_column.0),
                        )
                    }),
                    None,
                    SelectionLevel::Nested,
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        asql::{
            column_path::{ColumnPath, ColumnPathLink},
            predicate::AbstractPredicate,
            selection::{ColumnSelection, Selection, SelectionCardinality, SelectionElement},
        },
        sql::{predicate::Predicate, SQLParamContainer},
        transform::{
            pg::Postgres, test_util::TestSetup, transformer::SelectTransformer, SelectionLevel,
        },
        AbstractOrderBy, Limit, Offset, Ordering,
    };

    use super::AbstractSelect;
    use crate::sql::ExpressionBuilder;

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

                let select = Postgres {}.to_select(&aselect, None, None, SelectionLevel::TopLevel);
                assert_binding!(
                    select.into_sql(),
                    r#"SELECT "concerts"."id" FROM "concerts""#
                );
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
                let literal = ColumnPath::Literal(SQLParamContainer::new(5));
                let predicate = AbstractPredicate::Eq(concert_id_path, literal);

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

                let select = Postgres {}.to_select(&aselect, None, None, SelectionLevel::TopLevel);
                assert_binding!(
                    select.into_sql(),
                    r#"SELECT "concerts"."id" FROM "concerts" WHERE "concerts"."id" = $1"#,
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

                let select = Postgres {}.to_select(&aselect, None, None, SelectionLevel::TopLevel);
                assert_binding!(
                    select.into_sql(),
                    r#"SELECT COALESCE(json_agg(json_build_object('id', "concerts"."id")), '[]'::json)::text FROM "concerts""#
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

                let select = Postgres {}.to_select(&aselect, None, None, SelectionLevel::TopLevel);
                assert_binding!(
                    select.into_sql(),
                    r#"SELECT COALESCE(json_agg(json_build_object('id', "concerts"."id", 'venue', (SELECT json_build_object('id', "venues"."id") FROM "venues" WHERE "concerts"."venue_id" = "venues"."id"))), '[]'::json)::text FROM "concerts""#
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

                let select = Postgres {}.to_select(&aselect, None, None, SelectionLevel::TopLevel);
                assert_binding!(
                    select.into_sql(),
                    r#"SELECT COALESCE(json_agg(json_build_object('id', "venues"."id", 'concerts', (SELECT COALESCE(json_agg(json_build_object('id', "concerts"."id")), '[]'::json) FROM "concerts" WHERE "concerts"."venue_id" = "venues"."id"))), '[]'::json)::text FROM "venues""#
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
                    ]),
                    ColumnPath::Literal(SQLParamContainer::new("v1".to_string())),
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

                let select = Postgres {}.to_select(&aselect, None, None, SelectionLevel::TopLevel);
                assert_binding!(
                    select.into_sql(),
                    r#"SELECT COALESCE(json_agg(json_build_object('id', "concerts"."id")), '[]'::json)::text FROM "concerts" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id" WHERE "venues"."name" = $1"#,
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

                let select = Postgres {}.to_select(&aselect, None, None, SelectionLevel::TopLevel);
                assert_binding!(
                    select.into_sql(),
                    r#"SELECT "concerts"."id" FROM (SELECT "concerts".* FROM "concerts" ORDER BY "concerts"."name" ASC) AS "concerts""#
                );
            },
        );
    }

    #[test]
    fn with_predicate_limit_and_offset() {
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

                let literal = ColumnPath::Literal(SQLParamContainer::new("c1".to_string()));
                let predicate = AbstractPredicate::Eq(concert_name_path, literal);

                let aselect = AbstractSelect {
                    table: concerts_table,
                    selection: Selection::Seq(vec![ColumnSelection::new(
                        "id".to_string(),
                        SelectionElement::Physical(concerts_id_column),
                    )]),
                    predicate,
                    order_by: None,
                    offset: Some(Offset(10)),
                    limit: Some(Limit(20)),
                };

                let select = Postgres {}.to_select(&aselect, None, None, SelectionLevel::TopLevel);
                assert_binding!(
                    select.into_sql(),
                    r#"SELECT "concerts"."id" FROM (SELECT "concerts".* FROM "concerts" WHERE "concerts"."name" = $1 LIMIT $2 OFFSET $3) AS "concerts""#,
                    "c1".to_string(),
                    20i64,
                    10i64
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

                let select = Postgres {}.to_select(&aselect, None, None, SelectionLevel::TopLevel);
                assert_binding!(
                    select.into_sql(),
                    r#"SELECT "concerts"."id" FROM (SELECT "concerts".* FROM "concerts" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id" ORDER BY "venues"."name" ASC) AS "concerts""#
                );
            },
        );
    }
}
