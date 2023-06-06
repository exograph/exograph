// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{
    asql::column_path::{PhysicalColumnPathLink, RelationPhysicalColumnPathLink},
    sql::predicate::ConcretePredicate,
    transform::{pg::SelectionLevel, transformer::PredicateTransformer},
    AbstractPredicate, AbstractSelect, AliasedSelectionElement, Column, ColumnId, ColumnPath,
    Database, PhysicalColumnPath, Selection, SelectionElement,
};

use super::Postgres;

impl PredicateTransformer for Postgres {
    /// Transform an abstract predicate into a concrete predicate
    ///
    /// # Arguments
    /// * `predicate` - The predicate to transform
    /// * `tables_supplied` - Whether the tables are already in context. If they are, the predicate can simply use the table.column syntax.
    ///                       If they are not, the predicate will need to bring in the tables being referred to.
    fn to_predicate(
        &self,
        predicate: &AbstractPredicate,
        tables_supplied: bool,
        database: &Database,
    ) -> ConcretePredicate {
        if tables_supplied {
            to_join_predicate(predicate)
        } else {
            to_subselect_predicate(self, predicate, database)
        }
    }
}

/// Predicate that assumes that the tables are already in the context (perhaps through a join).
///
/// The predicate generated will look like "concerts.price = $1 AND venues.name = $2". It assumes
/// that the join would have brought in "concerts" and "venues" through a join.
fn to_join_predicate(predicate: &AbstractPredicate) -> ConcretePredicate {
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
fn to_subselect_predicate(
    transformer: &Postgres,
    predicate: &AbstractPredicate,
    database: &Database,
) -> ConcretePredicate {
    fn binary_operator(
        left: &ColumnPath,
        right: &ColumnPath,
        predicate_op: impl Fn(ColumnPath, ColumnPath) -> AbstractPredicate,
        database: &Database,
        select_transformer: &Postgres,
    ) -> Option<ConcretePredicate> {
        fn form_subselect(
            path: &ColumnPath,
            other: &ColumnPath,
            predicate_op: impl Fn(PhysicalColumnPath, ColumnPath) -> AbstractPredicate,
            database: &Database,
            select_transformer: &Postgres,
        ) -> Option<ConcretePredicate> {
            column_path_components(path).map(|(self_column_id, foreign_column_id, tail_links)| {
                let foreign_column = foreign_column_id.get_column(database);
                let abstract_select = AbstractSelect {
                    table_id: self_column_id.table_id,
                    selection: Selection::Seq(vec![AliasedSelectionElement::new(
                        foreign_column.name.clone(),
                        SelectionElement::Physical(foreign_column_id),
                    )]),
                    predicate: predicate_op(tail_links, other.clone()),
                    order_by: None,
                    offset: None,
                    limit: None,
                };

                let select = select_transformer.compute_select(
                    &abstract_select,
                    None,
                    SelectionLevel::Nested,
                    true, // allow duplicate rows to be returned since this is going to be used as a part of `IN`
                    database,
                );

                let select_column = Column::SubSelect(Box::new(select));

                ConcretePredicate::In(Column::Physical(self_column_id), select_column)
            })
        }

        form_subselect(
            left,
            right,
            |tail_links, right| predicate_op(ColumnPath::Physical(tail_links), right),
            database,
            select_transformer,
        )
        .or_else(|| {
            form_subselect(
                right,
                left,
                |tail_links, left| predicate_op(left, ColumnPath::Physical(tail_links)),
                database,
                select_transformer,
            )
        })
    }

    match predicate {
        AbstractPredicate::True => Some(ConcretePredicate::True),
        AbstractPredicate::False => Some(ConcretePredicate::False),

        AbstractPredicate::Eq(l, r) => {
            binary_operator(l, r, AbstractPredicate::Eq, database, transformer)
        }
        AbstractPredicate::Neq(l, r) => {
            binary_operator(l, r, AbstractPredicate::Neq, database, transformer)
        }
        AbstractPredicate::Lt(l, r) => {
            binary_operator(l, r, AbstractPredicate::Lt, database, transformer)
        }
        AbstractPredicate::Lte(l, r) => {
            binary_operator(l, r, AbstractPredicate::Lte, database, transformer)
        }
        AbstractPredicate::Gt(l, r) => {
            binary_operator(l, r, AbstractPredicate::Gt, database, transformer)
        }
        AbstractPredicate::Gte(l, r) => {
            binary_operator(l, r, AbstractPredicate::Gte, database, transformer)
        }
        AbstractPredicate::In(l, r) => {
            binary_operator(l, r, AbstractPredicate::In, database, transformer)
        }

        AbstractPredicate::StringStartsWith(l, r) => binary_operator(
            l,
            r,
            AbstractPredicate::StringStartsWith,
            database,
            transformer,
        ),
        AbstractPredicate::StringEndsWith(l, r) => binary_operator(
            l,
            r,
            AbstractPredicate::StringEndsWith,
            database,
            transformer,
        ),
        AbstractPredicate::StringLike(l, r, cs) => binary_operator(
            l,
            r,
            |l, r| AbstractPredicate::StringLike(l, r, *cs),
            database,
            transformer,
        ),
        AbstractPredicate::JsonContains(l, r) => {
            binary_operator(l, r, AbstractPredicate::JsonContains, database, transformer)
        }
        AbstractPredicate::JsonContainedBy(l, r) => binary_operator(
            l,
            r,
            AbstractPredicate::JsonContainedBy,
            database,
            transformer,
        ),
        AbstractPredicate::JsonMatchKey(l, r) => {
            binary_operator(l, r, AbstractPredicate::JsonMatchKey, database, transformer)
        }
        AbstractPredicate::JsonMatchAnyKey(l, r) => binary_operator(
            l,
            r,
            AbstractPredicate::JsonMatchAnyKey,
            database,
            transformer,
        ),
        AbstractPredicate::JsonMatchAllKeys(l, r) => binary_operator(
            l,
            r,
            AbstractPredicate::JsonMatchAllKeys,
            database,
            transformer,
        ),
        AbstractPredicate::And(p1, p2) => Some(ConcretePredicate::and(
            to_subselect_predicate(transformer, p1, database),
            to_subselect_predicate(transformer, p2, database),
        )),
        AbstractPredicate::Or(p1, p2) => Some(ConcretePredicate::or(
            to_subselect_predicate(transformer, p1, database),
            to_subselect_predicate(transformer, p2, database),
        )),
        AbstractPredicate::Not(p) => Some(ConcretePredicate::Not(Box::new(
            to_subselect_predicate(transformer, p, database),
        ))),
    }
    .unwrap_or(to_join_predicate(predicate)) // fallback to join predicate
}

fn leaf_column(column_path: &ColumnPath) -> Column {
    match column_path {
        ColumnPath::Physical(links) => Column::Physical(links.leaf_column()),
        ColumnPath::Param(l) => Column::Param(l.clone()),
        ColumnPath::Null => Column::Null,
    }
}

/// Returns the components of a column path that are relevant for a subselect predicate.
/// The first element of the tuple is the column in the root table, the second element is the
/// table that the column is linked to, the third element is the column in the linked table,
/// and the fourth element is the remaining links in the column path.
fn column_path_components(
    column_path: &ColumnPath,
) -> Option<(ColumnId, ColumnId, PhysicalColumnPath)> {
    match column_path {
        ColumnPath::Physical(links) => links.split_head().and_then(|(head, tail)| match head {
            PhysicalColumnPathLink::Relation(RelationPhysicalColumnPathLink {
                self_column_id,
                linked_column_id,
            }) => Some((self_column_id, linked_column_id, tail)),
            PhysicalColumnPathLink::Leaf(_) => None,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        sql::{predicate::CaseSensitivity, ExpressionBuilder, SQLParamContainer},
        transform::{pg::Postgres, test_util::TestSetup},
        AbstractPredicate, ColumnPath,
    };

    use super::*;

    #[test]
    fn non_nested_predicate() {
        TestSetup::with_setup(
            |TestSetup {
                 database,
                 concerts_name_column,
                 ..
             }| {
                let abstract_predicate = AbstractPredicate::Eq(
                    ColumnPath::Physical(PhysicalColumnPath::new(vec![
                        PhysicalColumnPathLink::Leaf(concerts_name_column),
                    ])),
                    ColumnPath::Param(SQLParamContainer::new("v1".to_string())),
                );

                {
                    let predicate = Postgres {}.to_predicate(&abstract_predicate, true, &database);
                    assert_binding!(
                        predicate.to_sql(&database),
                        r#""concerts"."name" = $1"#,
                        "v1".to_string()
                    );
                }

                {
                    let predicate = Postgres {}.to_predicate(&abstract_predicate, false, &database);
                    assert_binding!(
                        predicate.to_sql(&database),
                        r#""concerts"."name" = $1"#,
                        "v1".to_string()
                    );
                }
            },
        );
    }

    #[test]
    fn nested_op_predicate() {
        test_nested_op_predicate(AbstractPredicate::Eq, |l, r| format!("{l} = {r}"));
        test_nested_op_predicate(AbstractPredicate::Neq, |l, r| format!("{l} <> {r}"));
        test_nested_op_predicate(AbstractPredicate::Lt, |l, r| format!("{l} < {r}"));
        test_nested_op_predicate(AbstractPredicate::Lte, |l, r| format!("{l} <= {r}"));
        test_nested_op_predicate(AbstractPredicate::Gt, |l, r| format!("{l} > {r}"));
        test_nested_op_predicate(AbstractPredicate::Gte, |l, r| format!("{l} >= {r}"));
        test_nested_op_predicate(AbstractPredicate::In, |l, r| format!("{l} IN {r}"));

        test_nested_op_predicate(AbstractPredicate::StringStartsWith, |l, r| {
            format!("{l} LIKE {r} || '%'")
        });
        test_nested_op_predicate(AbstractPredicate::StringEndsWith, |l, r| {
            format!("{l} LIKE '%' || {r}")
        });
        test_nested_op_predicate(
            |l, r| AbstractPredicate::StringLike(l, r, CaseSensitivity::Insensitive),
            |l, r| format!("{l} ILIKE {r}"),
        );
        test_nested_op_predicate(
            |l, r| AbstractPredicate::StringLike(l, r, CaseSensitivity::Sensitive),
            |l, r| format!("{l} LIKE {r}"),
        );

        test_nested_op_predicate(AbstractPredicate::JsonContainedBy, |l, r| {
            format!("{l} <@ {r}")
        });
        test_nested_op_predicate(AbstractPredicate::JsonContains, |l, r| {
            format!("{l} @> {r}")
        });
        test_nested_op_predicate(AbstractPredicate::JsonMatchAllKeys, |l, r| {
            format!("{l} ?& {r}")
        });
        test_nested_op_predicate(AbstractPredicate::JsonMatchAnyKey, |l, r| {
            format!("{l} ?| {r}")
        });
        test_nested_op_predicate(AbstractPredicate::JsonMatchKey, |l, r| format!("{l} ? {r}"));
    }

    #[test]
    fn test_and() {
        TestSetup::with_setup(
            move |TestSetup {
                      database,
                      concerts_venue_id_column,
                      venues_id_column,
                      venues_name_column,
                      ..
                  }| {
                let abstract_predicate = AbstractPredicate::and(
                    AbstractPredicate::Eq(
                        ColumnPath::Physical(PhysicalColumnPath::new(vec![
                            PhysicalColumnPathLink::Leaf(concerts_venue_id_column),
                        ])),
                        ColumnPath::Param(SQLParamContainer::new(1)),
                    ),
                    AbstractPredicate::Eq(
                        ColumnPath::Physical(PhysicalColumnPath::new(vec![
                            PhysicalColumnPathLink::relation(
                                concerts_venue_id_column,
                                venues_id_column,
                            ),
                            PhysicalColumnPathLink::Leaf(venues_name_column),
                        ])),
                        ColumnPath::Param(SQLParamContainer::new("v1".to_string())),
                    ),
                );

                {
                    let predicate = Postgres {}.to_predicate(&abstract_predicate, true, &database);

                    assert_binding!(
                        predicate.to_sql(&database),
                        format!(r#"("concerts"."venue_id" = $1 AND "venues"."name" = $2)"#),
                        1,
                        "v1".to_string()
                    );
                }

                {
                    let predicate = Postgres {}.to_predicate(&abstract_predicate, false, &database);

                    assert_binding!(
                        predicate.to_sql(&database),
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
        OP: Clone + Fn(ColumnPath, ColumnPath) -> AbstractPredicate,
    {
        for literal_on_the_right in [true, false] {
            let op = op.clone();
            TestSetup::with_setup(
                move |TestSetup {
                          database,
                          concerts_venue_id_column,
                          venues_id_column,
                          venues_name_column,
                          ..
                      }| {
                    let physical_column = ColumnPath::Physical(PhysicalColumnPath::new(vec![
                        PhysicalColumnPathLink::relation(
                            concerts_venue_id_column,
                            venues_id_column,
                        ),
                        PhysicalColumnPathLink::Leaf(venues_name_column),
                    ]));
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
                        let predicate =
                            Postgres {}.to_predicate(&abstract_predicate, true, &database);
                        assert_binding!(
                            predicate.to_sql(&database),
                            predicate_stmt,
                            "v1".to_string()
                        );
                    }

                    {
                        let predicate =
                            Postgres {}.to_predicate(&abstract_predicate, false, &database);
                        let stmt = format!(
                            r#""concerts"."venue_id" IN (SELECT "venues"."id" FROM "venues" WHERE {predicate_stmt})"#
                        );
                        assert_binding!(predicate.to_sql(&database), stmt, "v1".to_string());
                    }
                },
            );
        }
    }
}
