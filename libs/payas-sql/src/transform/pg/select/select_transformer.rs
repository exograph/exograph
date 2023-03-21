use tracing::instrument;

use crate::{
    asql::select::AbstractSelect,
    sql::{
        predicate::ConcretePredicate,
        select::Select,
        sql_operation::SQLOperation,
        transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep},
    },
    transform::{transformer::SelectTransformer, SelectionLevel},
};

use super::{
    selection_context::SelectionContext, selection_strategy_chain::SelectionStrategyChain,
};

use crate::transform::pg::Postgres;

/// The current implementation makes the assumption that the return value of the select statement is a JSON object.
///
/// There are two axis to implement here:
/// 1. Return Aggregate: The assembly of the return value aggregate. This should match the shape of the return data in GraphQL query.
/// 2. Raw Data: Rows to feed into the return aggregate. This should match the data matching queries predicates, order by, limit, and offset.
///
/// Our current implementation decouples the two axis.
///
/// Consider the following GraphQL query (assuming the typical Concert/Venue schema):
/// ```graphql
/// {
///    concerts(where: {id: {gt: 10}}, orderBy: {id: asc}, limit: 10, offset: 20) {
///       id
///       title
///       venue {
///         id
///         name
///      }
///    }
/// }
/// ```
///
/// We will need to create a select statement with two components:
///
/// # Return Aggregate
///
/// Since the return value is a JSON array, we will use a `json_agg` to aggregate the rows into a JSON array. The `::text` cast is
/// necessary to convert the JSON array into a string, so that we the GraphQL query can just return the string as-is. See [`Select`]
/// for more details.
///
/// ```sql
/// COALESCE(
///     json_agg(
///         json_build_object(
///             'id', "concerts"."id",
///             'title', "concerts"."title",
///             'venue', (SELECT json_build_object(
///                 'id', "venues"."id",
///                 'name', "venues"."name") FROM "venues" WHERE "concerts"."venue_id" = "venues"."id")
///         )
///     ), '[]'::json
/// )::text
/// ```
///
/// If we were to return a single concert (for query such as `concert(id: 5)`), we would use a
/// `json_build_object` aggregate:
/// ```sql
/// SELECT json_build_object(
///    'id', "concerts"."id",
///    'title', "concerts"."title",
///    'venue', (SELECT json_build_object(
///       'id', "venues"."id",
///      'name', "venues"."name") FROM "venues" WHERE "concerts"."venue_id" = "venues"."id")
/// )::text FROM "concerts" WHERE "concerts"."id" = $1
/// ```
///
/// The forming of this json aggregate is done in [`selection_columns`] along with `[Selection`] and
/// [`SelectionElement`], so we won't discuss it here further. However, the important point here is
/// that the "Raw Data" part needs to return only the matching rows for the top-level table (in this
/// case, `concerts`). Any subfield of a relation (in this case, `venue`) will be handled by a
/// subselect in the aggregate formation. In our example, it will be handled by the subselect for
/// the `venue` field (note how it uses the `where` to pick up only the relevant venues).
///
/// # Raw data selection.
///
/// This is the selection of the rows that will be fed into the return aggregate. As mentioned
/// earlier, this should match the data matching queries predicates, order by, limit, and
/// offset--but only for the top-level table. An important consideration is making sure that we
/// don't return the same row more than once.
///
/// We first analyze the selection to determine characteristics of the selection:
/// - Does it use any order-by, limit or offset?
/// - Does it use any one-to-many clauses (in either predicate or order-by clause)?
///
/// We then use this information to determine the best way to select the raw data. See [`SelectionStrategyChain`]
/// for more details.
impl SelectTransformer for Postgres {
    /// Form a [`Select`] from a given [`AbstractSelect`].
    #[instrument(
        name = "SelectTransformer::to_select for Postgres"
        skip(self)
        )]
    fn to_select<'a>(&self, abstract_select: &AbstractSelect<'a>) -> Select<'a> {
        self.compute_select(abstract_select, None, SelectionLevel::TopLevel, false)
    }

    fn to_transaction_script<'a>(
        &self,
        abstract_select: &'a AbstractSelect,
    ) -> TransactionScript<'a> {
        let select = self.to_select(abstract_select);
        let mut transaction_script = TransactionScript::default();
        transaction_script.add_step(TransactionStep::Concrete(ConcreteTransactionStep::new(
            SQLOperation::Select(select),
        )));
        transaction_script
    }
}

impl Postgres {
    pub fn compute_select<'a>(
        &self,
        abstract_select: &AbstractSelect<'a>,
        additional_predicate: Option<ConcretePredicate<'a>>,
        selection_level: SelectionLevel,
        allow_duplicate_rows: bool,
    ) -> Select<'a> {
        let chain = SelectionStrategyChain::default();
        let selection_context = SelectionContext::new(
            abstract_select,
            additional_predicate,
            selection_level,
            allow_duplicate_rows,
            self,
        );
        chain.to_select(selection_context).unwrap()
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
        transform::{pg::Postgres, test_util::TestSetup, transformer::SelectTransformer},
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

                let select = Postgres {}.to_select(&aselect);
                assert_binding!(select.to_sql(), r#"SELECT "concerts"."id" FROM "concerts""#);
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

                let select = Postgres {}.to_select(&aselect);
                assert_binding!(
                    select.to_sql(),
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

                let select = Postgres {}.to_select(&aselect);
                assert_binding!(
                    select.to_sql(),
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

                let select = Postgres {}.to_select(&aselect);
                assert_binding!(
                    select.to_sql(),
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

                let select = Postgres {}.to_select(&aselect);
                assert_binding!(
                    select.to_sql(),
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

                let select = Postgres {}.to_select(&aselect);
                assert_binding!(
                    select.to_sql(),
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

                let select = Postgres {}.to_select(&aselect);
                assert_binding!(
                    select.to_sql(),
                    r#"SELECT "concerts"."id" FROM "concerts" ORDER BY "concerts"."name" ASC"#
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

                let select = Postgres {}.to_select(&aselect);
                assert_binding!(
                    select.to_sql(),
                    r#"SELECT "concerts"."id" FROM "concerts" WHERE "concerts"."name" = $1 LIMIT $2 OFFSET $3"#,
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

                let select = Postgres {}.to_select(&aselect);
                assert_binding!(
                    select.to_sql(),
                    r#"SELECT "concerts"."id" FROM "concerts" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id" ORDER BY "venues"."name" ASC"#
                );
            },
        );
    }
}
