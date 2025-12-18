// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{
    AbstractPredicate, AbstractSelect, AliasedSelectionElement, Column, ColumnPath, Database,
    NumericComparator, Selection, SelectionElement, VectorDistanceFunction,
    asql::column_path::{ColumnPathLink, RelationLink},
    sql::predicate::ConcretePredicate,
    transform::{pg::selection_level::SelectionLevel, transformer::PredicateTransformer},
};

use super::Postgres;

impl PredicateTransformer for Postgres {
    fn to_predicate(
        &self,
        predicate: &AbstractPredicate,
        selection_level: &SelectionLevel,
        tables_supplied: bool,
        database: &Database,
    ) -> ConcretePredicate {
        if tables_supplied {
            to_join_predicate(predicate, selection_level, database)
        } else {
            to_subselect_predicate(self, predicate, selection_level, database)
        }
    }
}

/// Predicate that assumes that the tables are already in the context (perhaps through a join).
///
/// The predicate generated will look like "concerts.price = $1 AND venues.name = $2". It assumes
/// that the join would have brought in "concerts" and "venues" through a join.
fn to_join_predicate(
    predicate: &AbstractPredicate,
    selection_level: &SelectionLevel,
    database: &Database,
) -> ConcretePredicate {
    let compute_leaf_column =
        |column_path: &ColumnPath| leaf_column(column_path, selection_level, database);

    match predicate {
        AbstractPredicate::True => ConcretePredicate::True,
        AbstractPredicate::False => ConcretePredicate::False,

        AbstractPredicate::Eq(l, r) => {
            ConcretePredicate::eq(compute_leaf_column(l), compute_leaf_column(r))
        }
        AbstractPredicate::Neq(l, r) => {
            ConcretePredicate::neq(compute_leaf_column(l), compute_leaf_column(r))
        }
        AbstractPredicate::Lt(l, r) => {
            ConcretePredicate::Lt(compute_leaf_column(l), compute_leaf_column(r))
        }
        AbstractPredicate::Lte(l, r) => {
            ConcretePredicate::Lte(compute_leaf_column(l), compute_leaf_column(r))
        }
        AbstractPredicate::Gt(l, r) => {
            ConcretePredicate::Gt(compute_leaf_column(l), compute_leaf_column(r))
        }
        AbstractPredicate::Gte(l, r) => {
            ConcretePredicate::Gte(compute_leaf_column(l), compute_leaf_column(r))
        }
        AbstractPredicate::In(l, r) => {
            ConcretePredicate::In(compute_leaf_column(l), compute_leaf_column(r))
        }

        AbstractPredicate::StringLike(l, r, cs) => {
            ConcretePredicate::StringLike(compute_leaf_column(l), compute_leaf_column(r), *cs)
        }
        AbstractPredicate::StringStartsWith(l, r) => {
            ConcretePredicate::StringStartsWith(compute_leaf_column(l), compute_leaf_column(r))
        }
        AbstractPredicate::StringEndsWith(l, r) => {
            ConcretePredicate::StringEndsWith(compute_leaf_column(l), compute_leaf_column(r))
        }

        AbstractPredicate::JsonContains(l, r) => {
            ConcretePredicate::JsonContains(compute_leaf_column(l), compute_leaf_column(r))
        }
        AbstractPredicate::JsonContainedBy(l, r) => {
            ConcretePredicate::JsonContainedBy(compute_leaf_column(l), compute_leaf_column(r))
        }
        AbstractPredicate::JsonMatchKey(l, r) => {
            ConcretePredicate::JsonMatchKey(compute_leaf_column(l), compute_leaf_column(r))
        }
        AbstractPredicate::JsonMatchAnyKey(l, r) => {
            ConcretePredicate::JsonMatchAnyKey(compute_leaf_column(l), compute_leaf_column(r))
        }
        AbstractPredicate::JsonMatchAllKeys(l, r) => {
            ConcretePredicate::JsonMatchAllKeys(compute_leaf_column(l), compute_leaf_column(r))
        }

        AbstractPredicate::VectorDistance(
            c1,
            c2,
            distance_op,
            numeric_comparator_op,
            threshold,
        ) => ConcretePredicate::VectorDistance(
            compute_leaf_column(c1),
            compute_leaf_column(c2),
            *distance_op,
            *numeric_comparator_op,
            compute_leaf_column(threshold),
        ),

        AbstractPredicate::And(l, r) => ConcretePredicate::and(
            to_join_predicate(l, selection_level, database),
            to_join_predicate(r, selection_level, database),
        ),
        AbstractPredicate::Or(l, r) => ConcretePredicate::or(
            to_join_predicate(l, selection_level, database),
            to_join_predicate(r, selection_level, database),
        ),
        AbstractPredicate::Not(p) => {
            ConcretePredicate::Not(Box::new(to_join_predicate(p, selection_level, database)))
        }
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
    selection_level: &SelectionLevel,
    database: &Database,
) -> ConcretePredicate {
    let debug = std::env::var("EXO_DEBUG_PREDICATE").is_ok();
    if debug {
        println!("[predicate-debug] entering to_subselect_predicate: {predicate:?}");
    }
    match attempt_subselect_predicate(predicate) {
        Some((relation_link, subselect_predicate)) => {
            if debug {
                println!(
                    "[predicate-debug] subselect via relation {relation_link:?} predicate {subselect_predicate:?}"
                );
            }
            form_subselect(relation_link, subselect_predicate, database, transformer)
        }
        None => match predicate {
            AbstractPredicate::And(p1, p2) => {
                if let (Some((link_l, pred_l)), Some((link_r, pred_r))) = (
                    attempt_subselect_predicate(p1),
                    attempt_subselect_predicate(p2),
                ) {
                    if link_l == link_r {
                        let combined = AbstractPredicate::and(pred_l, pred_r);
                        return form_subselect(link_l, combined, database, transformer);
                    }
                }

                ConcretePredicate::and(
                    to_subselect_predicate(transformer, p1, selection_level, database),
                    to_subselect_predicate(transformer, p2, selection_level, database),
                )
            }

            AbstractPredicate::Or(p1, p2) => ConcretePredicate::or(
                to_subselect_predicate(transformer, p1, selection_level, database),
                to_subselect_predicate(transformer, p2, selection_level, database),
            ),
            _ => to_join_predicate(predicate, selection_level, database),
        },
    }
}

fn form_subselect(
    relation_link: RelationLink,
    predicate: AbstractPredicate,
    database: &Database,
    select_transformer: &Postgres,
) -> ConcretePredicate {
    let selection = Selection::Seq(
        relation_link
            .column_pairs
            .iter()
            .map(|column_pair| {
                let foreign_column_id = column_pair.foreign_column_id;
                let foreign_column = foreign_column_id.get_column(database);
                AliasedSelectionElement::new(
                    foreign_column.name.clone(),
                    SelectionElement::Physical(foreign_column_id),
                )
            })
            .collect(),
    );

    let abstract_select = AbstractSelect {
        table_id: relation_link.self_table_id,
        selection,
        predicate,
        order_by: None,
        offset: None,
        limit: None,
    };

    let select = select_transformer.compute_select(
        abstract_select,
        &SelectionLevel::TopLevel,
        true, // allow duplicate rows to be returned since this is going to be used as a part of `IN`
        database,
    );

    let select_column = Column::SubSelect(Box::new(select));

    ConcretePredicate::In(
        Column::ColumnArray(
            relation_link
                .column_pairs
                .iter()
                .map(|column_pair| Column::physical(column_pair.self_column_id, None))
                .collect(),
        ),
        select_column,
    )
}

fn leaf_column(
    column_path: &ColumnPath,
    selection_level: &SelectionLevel,
    database: &Database,
) -> Column {
    match column_path {
        ColumnPath::Physical(links) => {
            let alias = match (selection_level.prefix(database), links.alias()) {
                (Some(prefix), Some(alias)) => Some(format!("{}${}", prefix, alias)),
                (None, Some(alias)) => Some(alias),
                _ => None,
            };
            Column::physical(links.leaf_column(), alias)
        }
        ColumnPath::Param(l) => Column::Param(l.clone()),
        ColumnPath::Null => Column::Null,
        ColumnPath::Predicate(_) => unreachable!(),
    }
}

fn attempt_subselect_predicate(
    predicate: &AbstractPredicate,
) -> Option<(RelationLink, AbstractPredicate)> {
    fn split(cp: &ColumnPath) -> Option<(RelationLink, ColumnPath)> {
        match cp {
            ColumnPath::Physical(physical_path) => {
                let (head, tail) = physical_path.split_head();
                match (head, tail) {
                    // If the tail is non-empty, the head will be a relation link (but the way we have expressed ColumnPath, we can't express that in the type system)
                    // TODO: Fix the type to express this
                    (ColumnPathLink::Relation(link), Some(tail)) => {
                        Some((link.clone(), ColumnPath::Physical(tail.clone())))
                    }
                    _ => None,
                }
            }
            ColumnPath::Param(_) | ColumnPath::Null | ColumnPath::Predicate(_) => None,
        }
    }

    fn binary_operator(
        l: &ColumnPath,
        r: &ColumnPath,
        constructor: impl Fn(ColumnPath, ColumnPath) -> AbstractPredicate,
    ) -> Option<(RelationLink, AbstractPredicate)> {
        match split(l) {
            Some((l_link, l_tail)) => match split(r) {
                Some((r_link, r_tail)) => {
                    // If both sides are linked paths, their links must be the same. In that case,
                    // compute the predicate based on their tails.
                    (l_link == r_link).then_some((l_link, constructor(l_tail, r_tail)))
                }
                None => Some((l_link, constructor(l_tail, r.clone()))),
            },
            None => split(r).map(|(r_link, r_tail)| (r_link, constructor(l.clone(), r_tail))),
        }
    }

    fn vector_distance_subselect_predicate(
        l: &ColumnPath,
        r: &ColumnPath,
        distance_function: &VectorDistanceFunction,
        comparator: &NumericComparator,
        comparator_path: &ColumnPath,
    ) -> Option<(RelationLink, AbstractPredicate)> {
        match (split(l), split(r), split(comparator_path)) {
            (None, None, None) => None,
            (None, None, Some((c_link, c_tail))) => Some((
                c_link,
                AbstractPredicate::VectorDistance(
                    l.clone(),
                    r.clone(),
                    *distance_function,
                    *comparator,
                    c_tail,
                ),
            )),
            (None, Some((r_link, r_tail)), None) => Some((
                r_link,
                AbstractPredicate::VectorDistance(
                    l.clone(),
                    r_tail,
                    *distance_function,
                    *comparator,
                    comparator_path.clone(),
                ),
            )),
            (None, Some((r_link, r_tail)), Some((c_link, c_tail))) => {
                (r_link == c_link).then_some((
                    r_link,
                    AbstractPredicate::VectorDistance(
                        l.clone(),
                        r_tail,
                        *distance_function,
                        *comparator,
                        c_tail,
                    ),
                ))
            }
            (Some((l_link, l_tail)), None, None) => Some((
                l_link,
                AbstractPredicate::VectorDistance(
                    l_tail,
                    r.clone(),
                    *distance_function,
                    *comparator,
                    comparator_path.clone(),
                ),
            )),
            (Some((l_link, l_tail)), None, Some((c_link, c_tail))) => {
                (l_link == c_link).then_some((
                    l_link,
                    AbstractPredicate::VectorDistance(
                        l_tail,
                        r.clone(),
                        *distance_function,
                        *comparator,
                        c_tail,
                    ),
                ))
            }
            (Some((l_link, l_tail)), Some((r_link, r_tail)), None) => {
                (l_link == r_link).then_some((
                    l_link,
                    AbstractPredicate::VectorDistance(
                        l_tail,
                        r_tail,
                        *distance_function,
                        *comparator,
                        comparator_path.clone(),
                    ),
                ))
            }
            (Some((l_link, l_tail)), Some((r_link, r_tail)), Some((c_link, c_tail))) => {
                (l_link == r_link && r_link == c_link).then_some((
                    l_link,
                    AbstractPredicate::VectorDistance(
                        l_tail,
                        r_tail,
                        *distance_function,
                        *comparator,
                        c_tail,
                    ),
                ))
            }
        }
    }

    fn logical_binary_op(
        l: &AbstractPredicate,
        r: &AbstractPredicate,
        constructor: impl Fn(Box<AbstractPredicate>, Box<AbstractPredicate>) -> AbstractPredicate,
    ) -> Option<(RelationLink, AbstractPredicate)> {
        attempt_subselect_predicate(l).and_then(|(l_link, l_path)| {
            attempt_subselect_predicate(r).and_then(|(r_link, r_path)| {
                (l_link == r_link)
                    .then_some((l_link, constructor(Box::new(l_path), Box::new(r_path))))
            })
        })
    }

    let result = match predicate {
        AbstractPredicate::True | AbstractPredicate::False => None,
        AbstractPredicate::Eq(l, r) => binary_operator(l, r, AbstractPredicate::Eq),
        AbstractPredicate::Neq(l, r) => binary_operator(l, r, AbstractPredicate::Neq),
        AbstractPredicate::Lt(l, r) => binary_operator(l, r, AbstractPredicate::Lt),
        AbstractPredicate::Lte(l, r) => binary_operator(l, r, AbstractPredicate::Lte),
        AbstractPredicate::Gt(l, r) => binary_operator(l, r, AbstractPredicate::Gt),
        AbstractPredicate::Gte(l, r) => binary_operator(l, r, AbstractPredicate::Gte),
        AbstractPredicate::In(l, r) => binary_operator(l, r, AbstractPredicate::In),
        AbstractPredicate::StringLike(l, r, sens) => {
            binary_operator(l, r, |l, r| AbstractPredicate::StringLike(l, r, *sens))
        }
        AbstractPredicate::StringStartsWith(l, r) => {
            binary_operator(l, r, AbstractPredicate::StringStartsWith)
        }
        AbstractPredicate::StringEndsWith(l, r) => {
            binary_operator(l, r, AbstractPredicate::StringEndsWith)
        }
        AbstractPredicate::JsonContains(l, r) => {
            binary_operator(l, r, AbstractPredicate::JsonContains)
        }
        AbstractPredicate::JsonContainedBy(l, r) => {
            binary_operator(l, r, AbstractPredicate::JsonContainedBy)
        }
        AbstractPredicate::JsonMatchKey(l, r) => {
            binary_operator(l, r, AbstractPredicate::JsonMatchKey)
        }
        AbstractPredicate::JsonMatchAnyKey(l, r) => {
            binary_operator(l, r, AbstractPredicate::JsonMatchAnyKey)
        }
        AbstractPredicate::JsonMatchAllKeys(l, r) => {
            binary_operator(l, r, AbstractPredicate::JsonMatchAllKeys)
        }

        AbstractPredicate::VectorDistance(l, r, distance_function, comparator, comparator_path) => {
            vector_distance_subselect_predicate(
                l,
                r,
                distance_function,
                comparator,
                comparator_path,
            )
        }

        AbstractPredicate::And(l, r) => logical_binary_op(l, r, AbstractPredicate::And),
        AbstractPredicate::Or(l, r) => logical_binary_op(l, r, AbstractPredicate::Or),
        AbstractPredicate::Not(p) => attempt_subselect_predicate(p)
            .map(|(p_link, p_path)| (p_link, AbstractPredicate::Not(Box::new(p_path)))),
    };

    result
}

#[cfg(test)]
mod tests {
    use crate::{
        AbstractPredicate, ColumnPath, PhysicalColumnPath,
        sql::{ExpressionBuilder, SQLParamContainer, predicate::CaseSensitivity},
        transform::{pg::Postgres, test_util::TestSetup},
    };

    use multiplatform_test::multiplatform_test;

    use super::*;

    #[multiplatform_test]
    fn non_nested_predicate() {
        TestSetup::with_setup(
            |TestSetup {
                 database,
                 concerts_name_column,
                 ..
             }| {
                let abstract_predicate = AbstractPredicate::Eq(
                    ColumnPath::Physical(PhysicalColumnPath::leaf(concerts_name_column)),
                    ColumnPath::Param(SQLParamContainer::string("v1".to_string())),
                );

                {
                    let predicate = Postgres {}.to_predicate(
                        &abstract_predicate,
                        &SelectionLevel::TopLevel,
                        true,
                        &database,
                    );
                    assert_binding!(
                        predicate.to_sql(&database),
                        r#""concerts"."name" = $1"#,
                        "v1".to_string()
                    );
                }

                {
                    let predicate = Postgres {}.to_predicate(
                        &abstract_predicate,
                        &SelectionLevel::TopLevel,
                        false,
                        &database,
                    );
                    assert_binding!(
                        predicate.to_sql(&database),
                        r#""concerts"."name" = $1"#,
                        "v1".to_string()
                    );
                }
            },
        );
    }

    #[multiplatform_test]
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

    #[multiplatform_test]
    fn test_and() {
        TestSetup::with_setup(
            move |TestSetup {
                      database,
                      concerts_venue_id_column,
                      venues_name_column,
                      ..
                  }| {
                let abstract_predicate = AbstractPredicate::and(
                    AbstractPredicate::Eq(
                        ColumnPath::Physical(PhysicalColumnPath::leaf(concerts_venue_id_column)),
                        ColumnPath::Param(SQLParamContainer::i32(1)),
                    ),
                    AbstractPredicate::Eq(
                        ColumnPath::Physical(PhysicalColumnPath::from_columns(
                            vec![concerts_venue_id_column, venues_name_column],
                            &database,
                        )),
                        ColumnPath::Param(SQLParamContainer::string("v1".to_string())),
                    ),
                );

                {
                    let predicate = Postgres {}.to_predicate(
                        &abstract_predicate,
                        &SelectionLevel::TopLevel,
                        true,
                        &database,
                    );

                    assert_binding!(
                        predicate.to_sql(&database),
                        format!(r#"("concerts"."venue_id" = $1 AND "venues"."name" = $2)"#),
                        1,
                        "v1".to_string()
                    );
                }

                {
                    let predicate = Postgres {}.to_predicate(
                        &abstract_predicate,
                        &SelectionLevel::TopLevel,
                        false,
                        &database,
                    );

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
                          venues_name_column,
                          ..
                      }| {
                    let physical_column = ColumnPath::Physical(PhysicalColumnPath::from_columns(
                        vec![concerts_venue_id_column, venues_name_column],
                        &database,
                    ));
                    let literal_column =
                        ColumnPath::Param(SQLParamContainer::string("v1".to_string()));

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
                        let predicate = Postgres {}.to_predicate(
                            &abstract_predicate,
                            &SelectionLevel::TopLevel,
                            true,
                            &database,
                        );
                        assert_binding!(
                            predicate.to_sql(&database),
                            predicate_stmt,
                            "v1".to_string()
                        );
                    }

                    {
                        let predicate = Postgres {}.to_predicate(
                            &abstract_predicate,
                            &SelectionLevel::TopLevel,
                            false,
                            &database,
                        );
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
