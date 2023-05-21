// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use tracing::instrument;

use crate::{
    asql::select::AbstractSelect,
    sql::{
        predicate::ConcretePredicate,
        select::Select,
        sql_operation::SQLOperation,
        transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep},
    },
    transform::{pg::SelectionLevel, transformer::SelectTransformer},
    Database,
};

use super::{
    selection_context::SelectionContext, selection_strategy_chain::SelectionStrategyChain,
};

use crate::transform::pg::Postgres;

/// Transform an abstract select into a select statement
///
/// There are two parts to implement here:
/// 1. Return Aggregate: The assembly of the return value. This should match the shape of the return
///    data in the GraphQL query or the columns specified in the `SELECT` clause.
/// 2. Raw Data: Rows to feed into the return aggregate. This should return the data matching the query's
///    predicate, order by, limit, and offset.
///
/// Our current implementation decouples the two parts.
///
/// Consider the following GraphQL query (assuming the typical `Venue -> [Concert]` schema):
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
/// Since the return value is a JSON array, we will use a `json_agg` to aggregate the rows into a
/// JSON array. The `::text` cast is necessary to convert the JSON array into a string, so that the
/// GraphQL query can just return the string obtained from the database as-is. See [`Select`] for
/// more details.
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
/// If we were to return a single concert (for a query such as `concert(id: 5)`), we would use a
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
/// earlier, this should return the data matching query's predicates, order by, limit, and
/// offset--but only for the top-level table. An important consideration is making sure that we
/// don't return the same row more than once.
///
/// We first analyze the selection to determine characteristics, such as:
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
    fn to_select<'a>(
        &self,
        abstract_select: &AbstractSelect<'a>,
        database: &'a Database,
    ) -> Select<'a> {
        self.compute_select(
            abstract_select,
            None,
            SelectionLevel::TopLevel,
            false,
            database,
        )
    }

    fn to_transaction_script<'a>(
        &self,
        abstract_select: &'a AbstractSelect,
        database: &'a Database,
    ) -> TransactionScript<'a> {
        let select = self.to_select(abstract_select, database);
        let mut transaction_script = TransactionScript::default();
        transaction_script.add_step(TransactionStep::Concrete(ConcreteTransactionStep::new(
            SQLOperation::Select(select),
        )));
        transaction_script
    }
}

impl Postgres {
    /// A lower-level version of [`to_select`] that allows for additional predicates and
    /// control over whether duplicate rows are allowed.
    pub fn compute_select<'a>(
        &self,
        abstract_select: &AbstractSelect<'a>,
        additional_predicate: Option<ConcretePredicate<'a>>,
        selection_level: SelectionLevel,
        allow_duplicate_rows: bool,
        database: &'a Database,
    ) -> Select<'a> {
        let selection_context = SelectionContext::new(
            database,
            abstract_select,
            additional_predicate,
            selection_level,
            allow_duplicate_rows,
            self,
        );
        let chain = SelectionStrategyChain::default();
        chain.to_select(selection_context, database).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        asql::{
            column_path::{ColumnIdPath, ColumnIdPathLink, ColumnPath},
            predicate::AbstractPredicate,
            selection::{
                AliasedSelectionElement, Selection, SelectionCardinality, SelectionElement,
            },
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
                 database,
                 concerts_table,
                 concerts_id_column,
                 ..
             }| {
                let aselect = AbstractSelect {
                    table_id: concerts_table,
                    selection: Selection::Seq(vec![AliasedSelectionElement::new(
                        "id".to_string(),
                        SelectionElement::Physical(concerts_id_column),
                    )]),
                    predicate: Predicate::True,
                    order_by: None,
                    offset: None,
                    limit: None,
                };

                let select = Postgres {}.to_select(&aselect, &database);
                assert_binding!(
                    select.to_sql(&database),
                    r#"SELECT "concerts"."id" FROM "concerts""#
                );
            },
        );
    }

    #[test]
    fn simple_predicate() {
        TestSetup::with_setup(
            |TestSetup {
                 database,
                 concerts_table,
                 concerts_id_column,
                 ..
             }| {
                let concert_id_path = ColumnPath::Physical(vec![ColumnIdPathLink {
                    self_column_id: concerts_id_column,
                    linked_column_id: None,
                }]);
                let literal = ColumnPath::Param(SQLParamContainer::new(5));
                let predicate = AbstractPredicate::Eq(concert_id_path, literal);

                let aselect = AbstractSelect {
                    table_id: concerts_table,
                    selection: Selection::Seq(vec![AliasedSelectionElement::new(
                        "id".to_string(),
                        SelectionElement::Physical(concerts_id_column),
                    )]),
                    predicate,
                    order_by: None,
                    offset: None,
                    limit: None,
                };

                let select = Postgres {}.to_select(&aselect, &database);
                assert_binding!(
                    select.to_sql(&database),
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
                 database,
                 concerts_table,
                 concerts_id_column,
                 ..
             }| {
                let aselect = AbstractSelect {
                    table_id: concerts_table,
                    selection: Selection::Json(
                        vec![AliasedSelectionElement::new(
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

                let select = Postgres {}.to_select(&aselect, &database);
                assert_binding!(
                    select.to_sql(&database),
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
                 database,
                 concerts_table,
                 venues_table,
                 concerts_id_column,
                 venues_id_column,
                 concerts_venue_id_column,
                 ..
             }| {
                let aselect = AbstractSelect {
                    table_id: concerts_table,
                    selection: Selection::Json(
                        vec![
                            AliasedSelectionElement::new(
                                "id".to_string(),
                                SelectionElement::Physical(concerts_id_column),
                            ),
                            AliasedSelectionElement::new(
                                "venue".to_string(),
                                SelectionElement::SubSelect(
                                    ColumnIdPathLink {
                                        self_column_id: concerts_venue_id_column,
                                        linked_column_id: Some(venues_id_column),
                                    },
                                    AbstractSelect {
                                        table_id: venues_table,
                                        selection: Selection::Json(
                                            vec![AliasedSelectionElement::new(
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

                let select = Postgres {}.to_select(&aselect, &database);
                assert_binding!(
                    select.to_sql(&database),
                    r#"SELECT COALESCE(json_agg(json_build_object('id', "concerts"."id", 'venue', (SELECT json_build_object('id', "venues"."id") FROM "venues" WHERE "concerts"."venue_id" = "venues"."id"))), '[]'::json)::text FROM "concerts""#
                );
            },
        );
    }

    #[test]
    fn nested_one_to_many_json() {
        TestSetup::with_setup(
            |TestSetup {
                 database,
                 concerts_table,
                 venues_table,
                 concerts_id_column,
                 venues_id_column,
                 concerts_venue_id_column,
                 ..
             }| {
                let aselect = AbstractSelect {
                    table_id: venues_table,
                    selection: Selection::Json(
                        vec![
                            AliasedSelectionElement::new(
                                "id".to_string(),
                                SelectionElement::Physical(venues_id_column),
                            ),
                            AliasedSelectionElement::new(
                                "concerts".to_string(),
                                SelectionElement::SubSelect(
                                    ColumnIdPathLink {
                                        self_column_id: concerts_venue_id_column,
                                        linked_column_id: Some(venues_id_column),
                                    },
                                    AbstractSelect {
                                        table_id: concerts_table,
                                        selection: Selection::Json(
                                            vec![AliasedSelectionElement::new(
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

                let select = Postgres {}.to_select(&aselect, &database);
                assert_binding!(
                    select.to_sql(&database),
                    r#"SELECT COALESCE(json_agg(json_build_object('id', "venues"."id", 'concerts', (SELECT COALESCE(json_agg(json_build_object('id', "concerts"."id")), '[]'::json) FROM "concerts" WHERE "concerts"."venue_id" = "venues"."id"))), '[]'::json)::text FROM "venues""#
                );
            },
        );
    }

    #[test]
    fn nested_predicate() {
        TestSetup::with_setup(
            |TestSetup {
                 database,
                 concerts_table,
                 concerts_id_column,
                 concerts_venue_id_column,
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
                        ColumnIdPathLink {
                            self_column_id: concerts_venue_id_column,
                            linked_column_id: Some(venues_id_column),
                        },
                        ColumnIdPathLink {
                            self_column_id: venues_name_column,
                            linked_column_id: None,
                        },
                    ]),
                    ColumnPath::Param(SQLParamContainer::new("v1".to_string())),
                );
                let aselect = AbstractSelect {
                    table_id: concerts_table,
                    selection: Selection::Json(
                        vec![AliasedSelectionElement::new(
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

                let select = Postgres {}.to_select(&aselect, &database);
                assert_binding!(
                    select.to_sql(&database),
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
                 database,
                 concerts_table,
                 concerts_id_column,
                 concerts_name_column,
                 ..
             }| {
                let concert_name_path = ColumnIdPath {
                    path: vec![ColumnIdPathLink {
                        self_column_id: concerts_name_column,
                        linked_column_id: None,
                    }],
                };

                let aselect = AbstractSelect {
                    table_id: concerts_table,
                    selection: Selection::Seq(vec![AliasedSelectionElement::new(
                        "id".to_string(),
                        SelectionElement::Physical(concerts_id_column),
                    )]),
                    predicate: Predicate::True,
                    order_by: Some(AbstractOrderBy(vec![(concert_name_path, Ordering::Asc)])),
                    offset: None,
                    limit: None,
                };

                let select = Postgres {}.to_select(&aselect, &database);
                assert_binding!(
                    select.to_sql(&database),
                    r#"SELECT "concerts"."id" FROM "concerts" ORDER BY "concerts"."name" ASC"#
                );
            },
        );
    }

    #[test]
    fn with_predicate_limit_and_offset() {
        TestSetup::with_setup(
            |TestSetup {
                 database,
                 concerts_table,
                 concerts_id_column,
                 concerts_name_column,
                 ..
             }| {
                let concert_name_path = ColumnPath::Physical(vec![ColumnIdPathLink {
                    self_column_id: concerts_name_column,
                    linked_column_id: None,
                }]);

                let literal = ColumnPath::Param(SQLParamContainer::new("c1".to_string()));
                let predicate = AbstractPredicate::Eq(concert_name_path, literal);

                let aselect = AbstractSelect {
                    table_id: concerts_table,
                    selection: Selection::Seq(vec![AliasedSelectionElement::new(
                        "id".to_string(),
                        SelectionElement::Physical(concerts_id_column),
                    )]),
                    predicate,
                    order_by: None,
                    offset: Some(Offset(10)),
                    limit: Some(Limit(20)),
                };

                let select = Postgres {}.to_select(&aselect, &database);
                assert_binding!(
                    select.to_sql(&database),
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
                 database,
                 concerts_table,
                 concerts_id_column,
                 venues_name_column,
                 concerts_venue_id_column,
                 venues_id_column,
                 ..
             }| {
                let venues_name_path = ColumnIdPath {
                    path: vec![
                        ColumnIdPathLink {
                            self_column_id: concerts_venue_id_column,
                            linked_column_id: Some(venues_id_column),
                        },
                        ColumnIdPathLink {
                            self_column_id: venues_name_column,
                            linked_column_id: None,
                        },
                    ],
                };

                let aselect = AbstractSelect {
                    table_id: concerts_table,
                    selection: Selection::Seq(vec![AliasedSelectionElement::new(
                        "id".to_string(),
                        SelectionElement::Physical(concerts_id_column),
                    )]),
                    predicate: Predicate::True,
                    order_by: Some(AbstractOrderBy(vec![(venues_name_path, Ordering::Asc)])),
                    offset: None,
                    limit: None,
                };

                let select = Postgres {}.to_select(&aselect, &database);
                assert_binding!(
                    select.to_sql(&database),
                    r#"SELECT "concerts"."id" FROM "concerts" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id" ORDER BY "venues"."name" ASC"#
                );
            },
        );
    }
}
