use crate::{
    asql::select::SelectionLevel,
    sql::predicate::ConcretePredicate,
    transform::transformer::{PredicateTransformer, SelectTransformer},
    AbstractPredicate, AbstractSelect, Column, ColumnPath, ColumnPathLink, ColumnSelection,
    PhysicalColumn, PhysicalTable, Selection, SelectionElement,
};

use super::Postgres;

impl PredicateTransformer for Postgres {
    fn to_predicate<'a>(&self, predicate: &AbstractPredicate<'a>) -> ConcretePredicate<'a> {
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
                ConcretePredicate::and(self.to_predicate(l), self.to_predicate(r))
            }
            AbstractPredicate::Or(l, r) => {
                ConcretePredicate::or(self.to_predicate(l), self.to_predicate(r))
            }
            AbstractPredicate::Not(p) => ConcretePredicate::Not(Box::new(self.to_predicate(p))),
        }
    }

    fn to_subselect_predicate<'a>(
        &self,
        predicate: &AbstractPredicate<'a>,
    ) -> ConcretePredicate<'a> {
        fn binary_operator<'p>(
            left: &ColumnPath<'p>,
            right: &ColumnPath<'p>,
            predicate_op: impl Fn(ColumnPath<'p>, ColumnPath<'p>) -> AbstractPredicate<'p>,
            select_transformer: &impl SelectTransformer,
        ) -> Option<ConcretePredicate<'p>> {
            match column_path_components(left) {
                Some((left_column, table, foreign_column, tail_links)) => {
                    let right_abstract_select = AbstractSelect {
                        table,
                        selection: Selection::Seq(vec![ColumnSelection {
                            column: SelectionElement::Physical(foreign_column),
                            alias: foreign_column.column_name.clone(),
                        }]),
                        predicate: predicate_op(
                            ColumnPath::Physical(tail_links.to_vec()),
                            right.clone(),
                        ),
                        order_by: None,
                        offset: None,
                        limit: None,
                    };

                    let right_select = select_transformer.to_select(
                        &right_abstract_select,
                        None,
                        None,
                        SelectionLevel::Nested,
                    );

                    let right_select_column = Column::SelectionTableWrapper(Box::new(right_select));

                    Some(ConcretePredicate::In(
                        Column::Physical(left_column),
                        right_select_column,
                    ))
                }
                None => None,
            }
        }

        match predicate {
            AbstractPredicate::True => Some(ConcretePredicate::True),
            AbstractPredicate::False => Some(ConcretePredicate::False),

            AbstractPredicate::Eq(l, r) => binary_operator(l, r, AbstractPredicate::Eq, self),
            AbstractPredicate::Neq(l, r) => binary_operator(l, r, AbstractPredicate::Neq, self),
            AbstractPredicate::Lt(l, r) => binary_operator(l, r, AbstractPredicate::Lt, self),
            AbstractPredicate::Lte(l, r) => binary_operator(l, r, AbstractPredicate::Lte, self),
            AbstractPredicate::Gt(l, r) => binary_operator(l, r, AbstractPredicate::Gt, self),
            AbstractPredicate::Gte(l, r) => binary_operator(l, r, AbstractPredicate::Gte, self),
            AbstractPredicate::In(l, r) => binary_operator(l, r, AbstractPredicate::In, self),

            AbstractPredicate::StringStartsWith(l, r) => {
                binary_operator(l, r, AbstractPredicate::StringStartsWith, self)
            }
            AbstractPredicate::StringEndsWith(l, r) => {
                binary_operator(l, r, AbstractPredicate::StringEndsWith, self)
            }
            AbstractPredicate::StringLike(l, r, cs) => {
                binary_operator(l, r, |l, r| AbstractPredicate::StringLike(l, r, *cs), self)
            }
            AbstractPredicate::JsonContains(l, r) => {
                binary_operator(l, r, AbstractPredicate::JsonContains, self)
            }
            AbstractPredicate::JsonContainedBy(l, r) => {
                binary_operator(l, r, AbstractPredicate::JsonContainedBy, self)
            }
            AbstractPredicate::JsonMatchKey(l, r) => {
                binary_operator(l, r, AbstractPredicate::JsonMatchKey, self)
            }
            AbstractPredicate::JsonMatchAnyKey(l, r) => {
                binary_operator(l, r, AbstractPredicate::JsonMatchAnyKey, self)
            }
            AbstractPredicate::JsonMatchAllKeys(l, r) => {
                binary_operator(l, r, AbstractPredicate::JsonMatchAllKeys, self)
            }
            AbstractPredicate::And(p1, p2) => Some(ConcretePredicate::and(
                self.to_subselect_predicate(p1),
                self.to_subselect_predicate(p2),
            )),
            AbstractPredicate::Or(p1, p2) => Some(ConcretePredicate::or(
                self.to_subselect_predicate(p1),
                self.to_subselect_predicate(p2),
            )),
            AbstractPredicate::Not(p) => Some(ConcretePredicate::Not(Box::new(
                self.to_subselect_predicate(p),
            ))),
        }
        .unwrap_or(self.to_predicate(predicate))
    }
}

fn leaf_column<'c>(column_path: &ColumnPath<'c>) -> Column<'c> {
    match column_path {
        ColumnPath::Physical(links) => Column::Physical(links.last().unwrap().self_column.0),
        ColumnPath::Literal(l) => Column::Literal(l.clone()),
        ColumnPath::Null => Column::Null,
    }
}

pub(super) fn column_path_components<'a, 'p>(
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
        sql::{predicate::CaseSensitivity, Expression, SQLParamContainer},
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
                    ColumnPath::Literal(SQLParamContainer::new("v1".to_string())),
                );

                {
                    let predicate = Postgres {}.to_predicate(&abstract_predicate);

                    let binding = predicate.binding();

                    assert_binding!(binding, r#""concerts"."name" = $1"#, "v1".to_string());
                }

                {
                    let predicate = Postgres {}.to_subselect_predicate(&abstract_predicate);

                    let binding = predicate.binding();

                    assert_binding!(binding, r#""concerts"."name" = $1"#, "v1".to_string());
                }
            },
        );
    }

    #[test]
    fn nested_op_predicate() {
        test_nested_op_predicate(|l, r| AbstractPredicate::Eq(l, r), "= $1");
        test_nested_op_predicate(|l, r| AbstractPredicate::Neq(l, r), "<> $1");
        test_nested_op_predicate(|l, r| AbstractPredicate::Lt(l, r), "< $1");
        test_nested_op_predicate(|l, r| AbstractPredicate::Lte(l, r), "<= $1");
        test_nested_op_predicate(|l, r| AbstractPredicate::Gt(l, r), "> $1");
        test_nested_op_predicate(|l, r| AbstractPredicate::Gte(l, r), ">= $1");
        test_nested_op_predicate(|l, r| AbstractPredicate::In(l, r), "IN $1");

        test_nested_op_predicate(
            |l, r| AbstractPredicate::StringStartsWith(l, r),
            "LIKE $1 || '%'",
        );
        test_nested_op_predicate(
            |l, r| AbstractPredicate::StringEndsWith(l, r),
            "LIKE '%' || $1",
        );
        test_nested_op_predicate(
            |l, r| AbstractPredicate::StringLike(l, r, CaseSensitivity::Insensitive),
            "ILIKE $1",
        );
        test_nested_op_predicate(
            |l, r| AbstractPredicate::StringLike(l, r, CaseSensitivity::Sensitive),
            "LIKE $1",
        );

        test_nested_op_predicate(|l, r| AbstractPredicate::JsonContainedBy(l, r), "<@ $1");
        test_nested_op_predicate(|l, r| AbstractPredicate::JsonContains(l, r), "@> $1");
        test_nested_op_predicate(|l, r| AbstractPredicate::JsonMatchAllKeys(l, r), "?& $1");
        test_nested_op_predicate(|l, r| AbstractPredicate::JsonMatchAnyKey(l, r), "?| $1");
        test_nested_op_predicate(|l, r| AbstractPredicate::JsonMatchKey(l, r), "? $1");
    }

    fn test_nested_op_predicate(
        op: impl for<'a> Fn(ColumnPath<'a>, ColumnPath<'a>) -> AbstractPredicate<'a>,
        sql_op: &'static str,
    ) {
        TestSetup::with_setup(
            move |TestSetup {
                      concerts_table,
                      concerts_venue_id_column,
                      venues_id_column,
                      venues_name_column,
                      venues_table,
                      ..
                  }| {
                let abstract_predicate = op(
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

                {
                    let predicate = Postgres {}.to_predicate(&abstract_predicate);

                    let binding = predicate.binding();

                    assert_binding!(
                        binding,
                        format!(r#""venues"."name" {sql_op}"#),
                        "v1".to_string()
                    );
                }

                {
                    let predicate = Postgres {}.to_subselect_predicate(&abstract_predicate);

                    let binding = predicate.binding();

                    assert_binding!(
                        binding,
                        format!(
                            r#""concerts"."venue_id" IN (SELECT "venues"."id" FROM "venues" WHERE "venues"."name" {sql_op})"#
                        ),
                        "v1".to_string()
                    );
                }
            },
        );
    }
}
