use crate::{
    sql::predicate::ConcretePredicate,
    transform::{pg::SelectionLevel, transformer::PredicateTransformer},
    AbstractPredicate, AbstractSelect, AliasedSelectionElement, Column, ColumnPath, ColumnPathLink,
    PhysicalColumn, PhysicalTable, Selection, SelectionElement,
};

use super::Postgres;

impl PredicateTransformer for Postgres {
    /// Transform an abstract predicate into a concrete predicate
    ///
    /// # Arguments
    /// * `predicate` - The predicate to transform
    /// * `tables_supplied` - Whether the tables are already in context. If they are, the predicate can simply use the table.column syntax.
    ///                       If they are not, the predicate will need to bring in the tables being referred to.
    fn to_predicate<'a>(
        &self,
        predicate: &AbstractPredicate<'a>,
        tables_supplied: bool,
    ) -> ConcretePredicate<'a> {
        if tables_supplied {
            to_join_predicate(predicate)
        } else {
            to_subselect_predicate(self, predicate)
        }
    }
}

/// Predicate that assumes that the tables are already in the context (perhaps through a join).
///
/// The predicate generated will look like "concerts.price = $1 AND venues.name = $2". It assumes
/// that the join would have brought in "concerts" and "venues" through a join.
fn to_join_predicate<'a>(predicate: &AbstractPredicate<'a>) -> ConcretePredicate<'a> {
    match predicate {
        AbstractPredicate::True => ConcretePredicate::True,
        AbstractPredicate::False => ConcretePredicate::False,

        AbstractPredicate::Eq(l, r) => ConcretePredicate::eq(leaf_column(l), leaf_column(r)),
        AbstractPredicate::Neq(l, r) => ConcretePredicate::neq(leaf_column(l), leaf_column(r)),
        AbstractPredicate::Lt(l, r) => ConcretePredicate::Lt(leaf_column(l), leaf_column(r)),
        AbstractPredicate::Lte(l, r) => ConcretePredicate::Lte(leaf_column(l), leaf_column(r)),
        AbstractPredicate::Gt(l, r) => ConcretePredicate::Gt(leaf_column(l), leaf_column(r)),
        AbstractPredicate::Gte(l, r) => ConcretePredicate::Gte(leaf_column(l), leaf_column(r)),
        AbstractPredicate::In(l, r) => ConcretePredicate::In(leaf_column(l), leaf_column(r)),

        AbstractPredicate::StringLike(l, r, cs) => {
            ConcretePredicate::StringLike(leaf_column(l), leaf_column(r), *cs)
        }
        AbstractPredicate::StringStartsWith(l, r) => {
            ConcretePredicate::StringStartsWith(leaf_column(l), leaf_column(r))
        }
        AbstractPredicate::StringEndsWith(l, r) => {
            ConcretePredicate::StringEndsWith(leaf_column(l), leaf_column(r))
        }

        AbstractPredicate::JsonContains(l, r) => {
            ConcretePredicate::JsonContains(leaf_column(l), leaf_column(r))
        }
        AbstractPredicate::JsonContainedBy(l, r) => {
            ConcretePredicate::JsonContainedBy(leaf_column(l), leaf_column(r))
        }
        AbstractPredicate::JsonMatchKey(l, r) => {
            ConcretePredicate::JsonMatchKey(leaf_column(l), leaf_column(r))
        }
        AbstractPredicate::JsonMatchAnyKey(l, r) => {
            ConcretePredicate::JsonMatchAnyKey(leaf_column(l), leaf_column(r))
        }
        AbstractPredicate::JsonMatchAllKeys(l, r) => {
            ConcretePredicate::JsonMatchAllKeys(leaf_column(l), leaf_column(r))
        }

        AbstractPredicate::And(l, r) => {
            ConcretePredicate::and(to_join_predicate(l), to_join_predicate(r))
        }
        AbstractPredicate::Or(l, r) => {
            ConcretePredicate::or(to_join_predicate(l), to_join_predicate(r))
        }
        AbstractPredicate::Not(p) => ConcretePredicate::Not(Box::new(to_join_predicate(p))),
    }
}

/// Predicate that doesn't assume that the tables are already in the context and it is this
/// predicate's job to bring in the tables being referred to.
///
/// A simplification is if the predicate is on the root table, which is always in the context, it
/// will lean on the join predicate.
///
/// So, if the abstract predicate is concerts.venue.name = "Theatre", the concrete predicate will
/// be:
///
/// ```sql
/// WHERE "concerts"."id" IN (SELECT "concerts"."id" FROM "concerts" JOIN "venues" ON "concerts"."venue_id" = "venues"."id" WHERE "venues"."name" = $1)
/// ```
/// However, if the abstract predicate is concerts.title = "Theatre", the concrete predicate will be
/// (because of the simplification):
///
/// ```sql
/// WHERE "concerts"."title" = $1
/// ```
///
/// Here, without the simplification, the concrete predicate would be:
/// ```sql
/// WHERE "concerts"."id" IN (SELECT "concerts"."id" FROM "concerts" WHERE "concerts"."title" = $1)
/// ```
/// which will be correct, but unnecessarily complex.
fn to_subselect_predicate<'a>(
    transformer: &Postgres,
    predicate: &AbstractPredicate<'a>,
) -> ConcretePredicate<'a> {
    println!("to_subselect_predicate: {:?}", predicate);
    fn binary_operator<'p>(
        left: &ColumnPath<'p>,
        right: &ColumnPath<'p>,
        predicate_op: impl Fn(ColumnPath<'p>, ColumnPath<'p>) -> AbstractPredicate<'p>,
        select_transformer: &Postgres,
    ) -> Option<ConcretePredicate<'p>> {
        fn form_subselect<'p>(
            path: &ColumnPath<'p>,
            other: &ColumnPath<'p>,
            predicate_op: impl Fn(Vec<ColumnPathLink<'p>>, ColumnPath<'p>) -> AbstractPredicate<'p>,
            select_transformer: &Postgres,
        ) -> Option<ConcretePredicate<'p>> {
            column_path_components(path).map(
                |(self_column, self_table, foreign_column, tail_links)| {
                    let abstract_select = AbstractSelect {
                        table: self_table,
                        selection: Selection::Seq(vec![AliasedSelectionElement::new(
                            foreign_column.name.clone(),
                            SelectionElement::Physical(foreign_column),
                        )]),
                        predicate: predicate_op(tail_links.to_vec(), other.clone()),
                        order_by: None,
                        offset: None,
                        limit: None,
                    };

                    let select = select_transformer.compute_select(
                        &abstract_select,
                        None,
                        SelectionLevel::Nested,
                        true, // allow duplicate rows to be returned since this is going to be used as a part of `IN`
                    );

                    let select_column = Column::SubSelect(Box::new(select));

                    ConcretePredicate::In(Column::Physical(self_column), select_column)
                },
            )
        }

        form_subselect(
            left,
            right,
            |tail_links, right| predicate_op(ColumnPath::Physical(tail_links), right),
            select_transformer,
        )
        .or_else(|| {
            form_subselect(
                right,
                left,
                |tail_links, left| {
                    predicate_op(left.clone(), ColumnPath::Physical(tail_links.to_vec()))
                },
                select_transformer,
            )
        })
    }

    match predicate {
        AbstractPredicate::True => Some(ConcretePredicate::True),
        AbstractPredicate::False => Some(ConcretePredicate::False),

        AbstractPredicate::Eq(l, r) => binary_operator(l, r, AbstractPredicate::Eq, transformer),
        AbstractPredicate::Neq(l, r) => binary_operator(l, r, AbstractPredicate::Neq, transformer),
        AbstractPredicate::Lt(l, r) => binary_operator(l, r, AbstractPredicate::Lt, transformer),
        AbstractPredicate::Lte(l, r) => binary_operator(l, r, AbstractPredicate::Lte, transformer),
        AbstractPredicate::Gt(l, r) => binary_operator(l, r, AbstractPredicate::Gt, transformer),
        AbstractPredicate::Gte(l, r) => binary_operator(l, r, AbstractPredicate::Gte, transformer),
        AbstractPredicate::In(l, r) => binary_operator(l, r, AbstractPredicate::In, transformer),

        AbstractPredicate::StringStartsWith(l, r) => {
            binary_operator(l, r, AbstractPredicate::StringStartsWith, transformer)
        }
        AbstractPredicate::StringEndsWith(l, r) => {
            binary_operator(l, r, AbstractPredicate::StringEndsWith, transformer)
        }
        AbstractPredicate::StringLike(l, r, cs) => binary_operator(
            l,
            r,
            |l, r| AbstractPredicate::StringLike(l, r, *cs),
            transformer,
        ),
        AbstractPredicate::JsonContains(l, r) => {
            binary_operator(l, r, AbstractPredicate::JsonContains, transformer)
        }
        AbstractPredicate::JsonContainedBy(l, r) => {
            binary_operator(l, r, AbstractPredicate::JsonContainedBy, transformer)
        }
        AbstractPredicate::JsonMatchKey(l, r) => {
            binary_operator(l, r, AbstractPredicate::JsonMatchKey, transformer)
        }
        AbstractPredicate::JsonMatchAnyKey(l, r) => {
            binary_operator(l, r, AbstractPredicate::JsonMatchAnyKey, transformer)
        }
        AbstractPredicate::JsonMatchAllKeys(l, r) => {
            binary_operator(l, r, AbstractPredicate::JsonMatchAllKeys, transformer)
        }
        AbstractPredicate::And(p1, p2) => Some(ConcretePredicate::and(
            to_subselect_predicate(transformer, p1),
            to_subselect_predicate(transformer, p2),
        )),
        AbstractPredicate::Or(p1, p2) => Some(ConcretePredicate::or(
            to_subselect_predicate(transformer, p1),
            to_subselect_predicate(transformer, p2),
        )),
        AbstractPredicate::Not(p) => Some(ConcretePredicate::Not(Box::new(
            to_subselect_predicate(transformer, p),
        ))),
    }
    .unwrap_or(to_join_predicate(predicate)) // fallback to join predicate
}

fn leaf_column<'c>(column_path: &ColumnPath<'c>) -> Column<'c> {
    match column_path {
        ColumnPath::Physical(links) => Column::Physical(links.last().unwrap().self_column.0),
        ColumnPath::Param(l) => Column::Param(l.clone()),
        ColumnPath::Null => Column::Null,
    }
}

/// Returns the components of a column path that are relevant for a subselect predicate.
/// The first element of the tuple is the column in the root table, the second element is the
/// table that the column is linked to, the third element is the column in the linked table,
/// and the fourth element is the remaining links in the column path.
fn column_path_components<'a, 'p>(
    column_path: &'a ColumnPath<'p>,
) -> Option<(
    &'p PhysicalColumn,
    &'p PhysicalTable,
    &'p PhysicalColumn,
    &'a [ColumnPathLink<'p>],
)> {
    match column_path {
        ColumnPath::Physical(links) => links.split_first().and_then(|(head, tail)| {
            head.linked_column.map(|linked_column| {
                (
                    head.self_column.0,
                    head.self_column.1,
                    linked_column.0,
                    tail,
                )
            })
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        sql::{predicate::CaseSensitivity, ExpressionBuilder, SQLParamContainer},
        transform::{pg::Postgres, test_util::TestSetup},
        AbstractPredicate, ColumnPath, ColumnPathLink,
    };

    use super::*;

    #[test]
    fn non_nested_predicate() {
        TestSetup::with_setup(
            |TestSetup {
                 concerts_table,
                 concerts_name_column,
                 ..
             }| {
                let abstract_predicate = AbstractPredicate::Eq(
                    ColumnPath::Physical(vec![ColumnPathLink {
                        self_column: (concerts_name_column, concerts_table),
                        linked_column: None,
                    }]),
                    ColumnPath::Param(SQLParamContainer::new("v1".to_string())),
                );

                {
                    let predicate = Postgres {}.to_predicate(&abstract_predicate, true);
                    assert_binding!(
                        predicate.to_sql(),
                        r#""concerts"."name" = $1"#,
                        "v1".to_string()
                    );
                }

                {
                    let predicate = Postgres {}.to_predicate(&abstract_predicate, false);
                    assert_binding!(
                        predicate.to_sql(),
                        r#""concerts"."name" = $1"#,
                        "v1".to_string()
                    );
                }
            },
        );
    }

    #[test]
    fn nested_op_predicate() {
        test_nested_op_predicate(
            |l, r| AbstractPredicate::Eq(l, r),
            |l, r| format!("{l} = {r}"),
        );
        test_nested_op_predicate(
            |l, r| AbstractPredicate::Neq(l, r),
            |l, r| format!("{l} <> {r}"),
        );
        test_nested_op_predicate(
            |l, r| AbstractPredicate::Lt(l, r),
            |l, r| format!("{l} < {r}"),
        );
        test_nested_op_predicate(
            |l, r| AbstractPredicate::Lte(l, r),
            |l, r| format!("{l} <= {r}"),
        );
        test_nested_op_predicate(
            |l, r| AbstractPredicate::Gt(l, r),
            |l, r| format!("{l} > {r}"),
        );
        test_nested_op_predicate(
            |l, r| AbstractPredicate::Gte(l, r),
            |l, r| format!("{l} >= {r}"),
        );
        test_nested_op_predicate(
            |l, r| AbstractPredicate::In(l, r),
            |l, r| format!("{l} IN {r}"),
        );

        test_nested_op_predicate(
            |l, r| AbstractPredicate::StringStartsWith(l, r),
            |l, r| format!("{l} LIKE {r} || '%'"),
        );
        test_nested_op_predicate(
            |l, r| AbstractPredicate::StringEndsWith(l, r),
            |l, r| format!("{l} LIKE '%' || {r}"),
        );
        test_nested_op_predicate(
            |l, r| AbstractPredicate::StringLike(l, r, CaseSensitivity::Insensitive),
            |l, r| format!("{l} ILIKE {r}"),
        );
        test_nested_op_predicate(
            |l, r| AbstractPredicate::StringLike(l, r, CaseSensitivity::Sensitive),
            |l, r| format!("{l} LIKE {r}"),
        );

        test_nested_op_predicate(
            |l, r| AbstractPredicate::JsonContainedBy(l, r),
            |l, r| format!("{l} <@ {r}"),
        );
        test_nested_op_predicate(
            |l, r| AbstractPredicate::JsonContains(l, r),
            |l, r| format!("{l} @> {r}"),
        );
        test_nested_op_predicate(
            |l, r| AbstractPredicate::JsonMatchAllKeys(l, r),
            |l, r| format!("{l} ?& {r}"),
        );
        test_nested_op_predicate(
            |l, r| AbstractPredicate::JsonMatchAnyKey(l, r),
            |l, r| format!("{l} ?| {r}"),
        );
        test_nested_op_predicate(
            |l, r| AbstractPredicate::JsonMatchKey(l, r),
            |l, r| format!("{l} ? {r}"),
        );
    }

    #[test]
    fn test_and() {
        TestSetup::with_setup(
            move |TestSetup {
                      concerts_table,
                      concerts_venue_id_column,
                      venues_id_column,
                      venues_name_column,
                      venues_table,
                      ..
                  }| {
                let abstract_predicate = AbstractPredicate::and(
                    AbstractPredicate::Eq(
                        ColumnPath::Physical(vec![ColumnPathLink {
                            self_column: (concerts_venue_id_column, concerts_table),
                            linked_column: None,
                        }]),
                        ColumnPath::Param(SQLParamContainer::new(1)),
                    ),
                    AbstractPredicate::Eq(
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
                        ColumnPath::Param(SQLParamContainer::new("v1".to_string())),
                    ),
                );

                {
                    let predicate = Postgres {}.to_predicate(&abstract_predicate, true);

                    assert_binding!(
                        predicate.to_sql(),
                        format!(r#"("concerts"."venue_id" = $1 AND "venues"."name" = $2)"#),
                        1,
                        "v1".to_string()
                    );
                }

                {
                    let predicate = Postgres {}.to_predicate(&abstract_predicate, false);

                    assert_binding!(
                        predicate.to_sql(),
                        format!(
                            r#"("concerts"."venue_id" = $1 AND "concerts"."venue_id" IN (SELECT "venues"."id" FROM "venues" WHERE "venues"."name" = $2))"#
                        ),
                        1,
                        "v1".to_string()
                    );
                }
            },
        );
    }

    fn test_nested_op_predicate<OP>(op: OP, op_combinator: fn(&str, &str) -> String)
    where
        OP: Clone + for<'a> Fn(ColumnPath<'a>, ColumnPath<'a>) -> AbstractPredicate<'a>,
    {
        for literal_on_the_right in [true, false] {
            let op = op.clone();
            TestSetup::with_setup(
                move |TestSetup {
                          concerts_table,
                          concerts_venue_id_column,
                          venues_id_column,
                          venues_name_column,
                          venues_table,
                          ..
                      }| {
                    let physical_column = ColumnPath::Physical(vec![
                        ColumnPathLink {
                            self_column: (concerts_venue_id_column, concerts_table),
                            linked_column: Some((venues_id_column, venues_table)),
                        },
                        ColumnPathLink {
                            self_column: (venues_name_column, venues_table),
                            linked_column: None,
                        },
                    ]);
                    let literal_column =
                        ColumnPath::Param(SQLParamContainer::new("v1".to_string()));

                    let abstract_predicate = if literal_on_the_right {
                        op(physical_column, literal_column)
                    } else {
                        op(literal_column, physical_column)
                    };

                    let predicate_stmt = if literal_on_the_right {
                        op_combinator(r#""venues"."name""#, "$1")
                    } else {
                        op_combinator("$1", r#""venues"."name""#)
                    };

                    {
                        let predicate = Postgres {}.to_predicate(&abstract_predicate, true);
                        assert_binding!(predicate.to_sql(), predicate_stmt, "v1".to_string());
                    }

                    {
                        let predicate = Postgres {}.to_predicate(&abstract_predicate, false);
                        let stmt = format!(
                            r#""concerts"."venue_id" IN (SELECT "venues"."id" FROM "venues" WHERE {predicate_stmt})"#
                        );
                        assert_binding!(predicate.to_sql(), stmt, "v1".to_string());
                    }
                },
            );
        }
    }
}
