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
    ///
    /// The implementation makes the assumption that the return value of the select statement is a JSON object.
    ///
    /// There are two axis to deal with here:
    /// - The assembly of the return value
    /// - The selection of the rows to assemble the return value from
    ///
    /// So for example, if we have a GraphQL query like:
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
    /// We will need form:
    /// - A json aggregate to assemble the return value. In this case, something like:
    /// ```sql
    /// COALESCE(
    ///     json_agg(
    ///         json_build_object(
    ///             'id', "concerts"."id",
    ///             'title', "concerts"."title",
    ///             'venue', (SELECT json_build_object(
    ///                 'id', "venues"."id",
    ///                 'name', "venues"."name") FROM "venues" WHERE "concerts"."venueid" = "venues"."id")
    ///         )
    ///     ), '[]'::json
    /// )::text
    /// ```
    ///
    /// # Implementation notes:
    ///
    /// We first analyze the selection to determine characteristics of the selection:
    /// - Does it use any order-by, limit or offset?
    /// - Does it use any one-to-many clauses (in either predicate or order-by clause)?
    ///
    /// ## Single table selection without order-by, limit or offset
    ///
    /// The simplest case is when the selection is a single table selection with possibly a
    /// predicate but **without** any order-by, limit, or offset. In this case, we can simply return
    /// a direct select statement (no subselect or join). Typically, we will produce a statement
    /// like:
    ///
    /// - For a single row selection:
    /// ```sql
    /// SELECT json_build_object('id', "concerts"."id")::text FROM "concerts" WHERE "concerts"."id" = $1
    /// ```
    /// - For a multiple rows selection:
    /// ```sql
    /// SELECT COALESCE(json_agg(json_build_object('id', "concerts"."id")), '[]'::json)::text FROM "concerts" WHERE "concerts"."id" > $1
    /// ```
    ///
    /// ## Single table selection with order-by, limit or offset
    ///
    /// When the selection has an order-by, limit or offset, we need to use a subselect so that we can use
    /// order-by, limit and offset (which don't make sense when the return value is an aggregate (like a `json_agg`)).
    ///
    /// Since we can provide an order-by, limit, or offset cause only to a multi-row selection, we need not worry about the single-row case.
    ///
    /// Here we produce a statement like:
    ///
    /// ```sql
    /// SELECT COALESCE(json_agg(json_build_object('id', "concerts"."id")), '[]'::json)::text FROM (
    ///     SELECT "concerts".* FROM "concerts" WHERE "concerts"."id" > $1 LIMIT $2 OFFSET $3
    /// ) AS "concerts"
    /// ```
    /// It is the subselect's job to apply the order-by, limit and offset and return all the columns of the table.
    /// (A possible optimization would be to only return the columns that are needed by the selection.)
    ///
    ///
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

        // let is_single_table_select = columns_paths.iter().any(|path| path.len() == 1);

        // if is_single_table_select {
        //     return single_table_select(
        //         abstract_select,
        //         additional_predicate,
        //         selection_level,
        //         self,
        //         self,
        //     );
        // }

        let has_top_many_to_one_clause = columns_paths
            .iter()
            .any(|path| path[0].self_column.0.is_pk && path[0].linked_column.is_some());

        let has_a_many_to_one_clause = columns_paths.iter().any(|path| {
            path.iter()
                .any(|link| link.self_column.0.is_pk && link.linked_column.is_some())
        });

        let join = join_util::compute_join(abstract_select.table, columns_paths);

        println!("Join: {:?}", crate::sql::ExpressionBuilder::to_sql(&join));

        let selection_columns = abstract_select.selection.selection_columns(self);

        let has_non_predicate_clauses = abstract_select.order_by.is_some()
            || abstract_select.offset.is_some()
            || abstract_select.limit.is_some();

        dbg!(
            has_top_many_to_one_clause,
            has_a_many_to_one_clause,
            has_non_predicate_clauses
        );

        if has_a_many_to_one_clause || has_non_predicate_clauses {
            let inner_select = {
                let (predicate, selection_table_query) =
                    if has_top_many_to_one_clause && selection_level == SelectionLevel::TopLevel {
                        // If we have a many to one clause, we need to use a subselect along with
                        // the basic table (not a join) to avoid returning duplicate rows (that would be returned by the join)
                        (
                            self.to_subselect_predicate(&abstract_select.predicate),
                            Table::Physical(abstract_select.table),
                        )
                    } else if has_a_many_to_one_clause {
                        let predicate = self.to_join_predicate(&abstract_select.predicate);

                        let inner_select = Select {
                            table: join,
                            columns: vec![Column::Physical(
                                abstract_select.table.get_pk_physical_column().unwrap(),
                            )],
                            predicate: ConcretePredicate::In(
                                Column::Physical(
                                    abstract_select.table.get_pk_physical_column().unwrap(),
                                ),
                                Column::SubSelect(Box::new(Select {
                                    table: Table::Physical(abstract_select.table),
                                    columns: vec![Column::Physical(
                                        abstract_select.table.get_pk_physical_column().unwrap(),
                                    )],
                                    predicate,
                                    order_by: abstract_select
                                        .order_by
                                        .as_ref()
                                        .map(|ob| self.to_order_by(ob)),
                                    offset: abstract_select.offset.clone(),
                                    limit: abstract_select.limit.clone(),
                                    group_by: None,
                                    top_level_selection: false,
                                })),
                            ),
                            order_by: None,
                            offset: None,
                            limit: None,
                            group_by: None,
                            top_level_selection: false,
                        };

                        (
                            ConcretePredicate::In(
                                Column::Physical(
                                    abstract_select.table.get_pk_physical_column().unwrap(),
                                ),
                                Column::SubSelect(Box::new(inner_select)),
                            ),
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
                    top_level_selection: false,
                }
            };

            // We then use the inner select to build the final select
            Select {
                table: Table::SubSelect {
                    select: Box::new(inner_select),
                    alias: Some(abstract_select.table.name.clone()),
                },
                columns: selection_columns,
                predicate: ConcretePredicate::True,
                order_by: None,
                offset: None,
                limit: None,
                group_by: None,
                top_level_selection: selection_level == SelectionLevel::TopLevel,
            }
        } else {
            let predicate = ConcretePredicate::and(
                self.to_join_predicate(&abstract_select.predicate),
                additional_predicate.unwrap_or(ConcretePredicate::True),
            );

            let x = Select {
                table: join,
                columns: selection_columns,
                predicate,
                order_by: abstract_select
                    .order_by
                    .as_ref()
                    .map(|ob| self.to_order_by(ob)),
                offset: abstract_select.offset.clone(),
                limit: abstract_select.limit.clone(),
                group_by,
                top_level_selection: selection_level == SelectionLevel::TopLevel,
            };

            println!("Select: {:?}", crate::sql::ExpressionBuilder::to_sql(&x));
            x
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

// Compute select for a simple query that involves only one table.
// This common case is optimized to avoid the joins or subselects.
// fn single_table_select<'a>(
//     abstract_select: &AbstractSelect<'a>,
//     additional_predicate: Option<ConcretePredicate<'a>>,
//     selection_level: SelectionLevel,
//     select_transformer: &impl SelectTransformer,
//     predicate_transformer: &impl PredicateTransformer,
//     order_by_transformer: &impl OrderByTransformer,
// ) -> Select<'a> {
//     let predicate: crate::Predicate<Column<'a>> = ConcretePredicate::and(
//         predicate_transformer.to_join_predicate(&abstract_select.predicate),
//         additional_predicate.unwrap_or(ConcretePredicate::True),
//     );

//     Select {
//         table: Table::Physical(abstract_select.table),
//         columns: abstract_select
//             .selection
//             .selection_columns(select_transformer),
//         predicate,
//         order_by: abstract_select
//             .order_by
//             .as_ref()
//             .map(|ob| order_by_transformer.to_order_by(ob)),
//         offset: abstract_select.offset.clone(),
//         limit: abstract_select.limit.clone(),
//         group_by: None,
//         top_level_selection: selection_level == SelectionLevel::TopLevel,
//     }
// }

impl<'a> Selection<'a> {
    pub fn to_sql(&self, select_transformer: &impl SelectTransformer) -> SelectionSQL<'a> {
        match self {
            Selection::Seq(seq) => SelectionSQL::Seq(
                seq.iter()
                    .map(
                        |ColumnSelection {
                             alias: _alias,
                             column,
                         }| column.to_sql(select_transformer),
                    )
                    .collect(),
            ),
            Selection::Json(seq, cardinality) => {
                let object_elems = seq
                    .iter()
                    .map(|ColumnSelection { alias, column }| {
                        JsonObjectElement::new(alias.clone(), column.to_sql(select_transformer))
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

    pub fn selection_columns(
        &self,
        select_transformer: &impl SelectTransformer,
    ) -> Vec<Column<'a>> {
        match self.to_sql(select_transformer) {
            SelectionSQL::Single(elem) => vec![elem],
            SelectionSQL::Seq(elems) => elems,
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

                let select = Postgres {}.to_select(&aselect, None, None, SelectionLevel::TopLevel);
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

                let select = Postgres {}.to_select(&aselect, None, None, SelectionLevel::TopLevel);
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

                let select = Postgres {}.to_select(&aselect, None, None, SelectionLevel::TopLevel);
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

                let select = Postgres {}.to_select(&aselect, None, None, SelectionLevel::TopLevel);
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

                let select = Postgres {}.to_select(&aselect, None, None, SelectionLevel::TopLevel);
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

                let select = Postgres {}.to_select(&aselect, None, None, SelectionLevel::TopLevel);
                assert_binding!(
                    select.to_sql(),
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
                    select.to_sql(),
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
                    select.to_sql(),
                    r#"SELECT "concerts"."id" FROM (SELECT "concerts".* FROM "concerts" LEFT JOIN "venues" ON "concerts"."venue_id" = "venues"."id" ORDER BY "venues"."name" ASC) AS "concerts""#
                );
            },
        );
    }
}
