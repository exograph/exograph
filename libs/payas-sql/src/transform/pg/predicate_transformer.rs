use maybe_owned::MaybeOwned;

use crate::{
    asql::select::SelectionLevel,
    sql::group_by::GroupBy,
    transform::transformer::{PredicateTransformer, SelectTransformer},
    AbstractPredicate, AbstractSelect, Column, ColumnPath, ColumnPathLink, ColumnSelection,
    PhysicalColumn, PhysicalTable, Predicate, Selection, SelectionElement,
};

use super::Postgres;

impl PredicateTransformer for Postgres {
    fn to_predicate<'a>(&self, predicate: &AbstractPredicate<'a>) -> crate::Predicate<'a> {
        match predicate {
            AbstractPredicate::True => Predicate::True,
            AbstractPredicate::False => Predicate::False,

            AbstractPredicate::Eq(l, r) => {
                Predicate::eq(leaf_column(l).into(), leaf_column(r).into())
            }
            AbstractPredicate::Neq(l, r) => {
                Predicate::neq(leaf_column(l).into(), leaf_column(r).into())
            }
            AbstractPredicate::Lt(l, r) => {
                Predicate::Lt(leaf_column(l).into(), leaf_column(r).into())
            }
            AbstractPredicate::Lte(l, r) => {
                Predicate::Lte(leaf_column(l).into(), leaf_column(r).into())
            }
            AbstractPredicate::Gt(l, r) => {
                Predicate::Gt(leaf_column(l).into(), leaf_column(r).into())
            }
            AbstractPredicate::Gte(l, r) => {
                Predicate::Gte(leaf_column(l).into(), leaf_column(r).into())
            }
            AbstractPredicate::In(l, r) => {
                Predicate::In(leaf_column(l).into(), leaf_column(r).into())
            }

            AbstractPredicate::StringLike(l, r, cs) => {
                Predicate::StringLike(leaf_column(l).into(), leaf_column(r).into(), *cs)
            }
            AbstractPredicate::StringStartsWith(l, r) => {
                Predicate::StringStartsWith(leaf_column(l).into(), leaf_column(r).into())
            }
            AbstractPredicate::StringEndsWith(l, r) => {
                Predicate::StringEndsWith(leaf_column(l).into(), leaf_column(r).into())
            }

            AbstractPredicate::JsonContains(l, r) => {
                Predicate::JsonContains(leaf_column(l).into(), leaf_column(r).into())
            }
            AbstractPredicate::JsonContainedBy(l, r) => {
                Predicate::JsonContainedBy(leaf_column(l).into(), leaf_column(r).into())
            }
            AbstractPredicate::JsonMatchKey(l, r) => {
                Predicate::JsonMatchKey(leaf_column(l).into(), leaf_column(r).into())
            }
            AbstractPredicate::JsonMatchAnyKey(l, r) => {
                Predicate::JsonMatchAnyKey(leaf_column(l).into(), leaf_column(r).into())
            }
            AbstractPredicate::JsonMatchAllKeys(l, r) => {
                Predicate::JsonMatchAllKeys(leaf_column(l).into(), leaf_column(r).into())
            }

            AbstractPredicate::And(l, r) => {
                Predicate::and(self.to_predicate(l), self.to_predicate(r))
            }
            AbstractPredicate::Or(l, r) => {
                Predicate::or(self.to_predicate(l), self.to_predicate(r))
            }
            AbstractPredicate::Not(p) => Predicate::Not(Box::new(self.to_predicate(p))),
        }
    }

    fn to_subselect_predicate<'a>(&self, predicate: &'a AbstractPredicate<'a>) -> Predicate<'a> {
        fn binary_operator<'a>(
            left: &'a ColumnPath<'a>,
            right: &'a ColumnPath<'a>,
            predicate_op: impl Fn(
                MaybeOwned<'a, ColumnPath<'a>>,
                MaybeOwned<'a, ColumnPath<'a>>,
            ) -> AbstractPredicate<'a>,
            select_transformer: &impl SelectTransformer,
        ) -> Option<Predicate<'a>> {
            match components(left) {
                Some((left_column, table, foreign_column, tail_links)) => {
                    let right_abstract_select = AbstractSelect {
                        table,
                        selection: Selection::Seq(vec![ColumnSelection {
                            column: SelectionElement::Physical(foreign_column),
                            alias: foreign_column.column_name.clone(),
                        }]),
                        predicate: predicate_op(
                            MaybeOwned::Owned(ColumnPath::Physical(tail_links.to_vec())),
                            MaybeOwned::Borrowed(right),
                        ),
                        order_by: None,
                        offset: None,
                        limit: None,
                    };

                    let right_select = select_transformer.to_select(
                        &right_abstract_select,
                        None,
                        Some(GroupBy(vec![foreign_column])),
                        SelectionLevel::Nested,
                    );

                    let right_select_column = Column::SelectionTableWrapper(Box::new(right_select));

                    Some(Predicate::In(
                        Column::Physical(left_column).into(),
                        right_select_column.into(),
                    ))
                }
                None => None,
            }
        }

        match predicate {
            AbstractPredicate::True => Some(Predicate::True),
            AbstractPredicate::False => Some(Predicate::False),

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

            AbstractPredicate::And(p1, p2) => Some(Predicate::and(
                self.to_subselect_predicate(p1),
                self.to_subselect_predicate(p2),
            )),
            AbstractPredicate::Or(p1, p2) => Some(Predicate::or(
                self.to_subselect_predicate(p1),
                self.to_subselect_predicate(p2),
            )),
            AbstractPredicate::Not(p) => {
                Some(Predicate::Not(Box::new(self.to_subselect_predicate(p))))
            }
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

fn components<'a, 'p>(
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

    use maybe_owned::MaybeOwned;

    use crate::{
        sql::{predicate::CaseSensitivity, Expression, ExpressionContext, SQLParamContainer},
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
                    }])
                    .into(),
                    ColumnPath::Literal(SQLParamContainer::new("v1".to_string())).into(),
                );

                {
                    let predicate = Postgres {}.to_predicate(&abstract_predicate);
                    let mut expr = ExpressionContext::default();
                    let binding = predicate.binding(&mut expr);

                    assert_binding!(binding, r#""concerts"."name" = $1"#, "v1".to_string());
                }

                {
                    let predicate = Postgres {}.to_subselect_predicate(&abstract_predicate);
                    let mut expr = ExpressionContext::default();
                    let binding = predicate.binding(&mut expr);

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
        op: impl for<'a> Fn(
            MaybeOwned<'a, ColumnPath<'a>>,
            MaybeOwned<'a, ColumnPath<'a>>,
        ) -> AbstractPredicate<'a>,
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
                    ])
                    .into(),
                    ColumnPath::Literal(SQLParamContainer::new("v1".to_string())).into(),
                );

                {
                    let predicate = Postgres {}.to_predicate(&abstract_predicate);
                    let mut expr = ExpressionContext::default();
                    let binding = predicate.binding(&mut expr);

                    assert_binding!(
                        binding,
                        format!(r#""venues"."name" {sql_op}"#),
                        "v1".to_string()
                    );
                }

                {
                    let predicate = Postgres {}.to_subselect_predicate(&abstract_predicate);
                    let mut expr = ExpressionContext::default();
                    let binding = predicate.binding(&mut expr);

                    assert_binding!(
                        binding,
                        format!(
                            r#""concerts"."venue_id" IN (select "venues"."id" from "venues" WHERE "venues"."name" {sql_op} GROUP BY "venues"."id")"#
                        ),
                        "v1".to_string()
                    );
                }
            },
        );
    }
}
