use maybe_owned::MaybeOwned;

use crate::{
    asql::select::SelectionLevel,
    sql::{column::Column, predicate::Predicate},
    transform::transformer::SelectTransformer,
    AbstractSelect, ColumnPathLink, ColumnSelection, PhysicalColumn, PhysicalTable, Selection,
    SelectionElement,
};

use super::column_path::ColumnPath;

pub type AbstractPredicate<'a> = Predicate<'a, ColumnPath<'a>>;

impl<'a> AbstractPredicate<'a> {
    pub fn column_paths(&self) -> Vec<&ColumnPath<'a>> {
        match self {
            AbstractPredicate::True | AbstractPredicate::False => vec![],
            AbstractPredicate::Eq(l, r)
            | AbstractPredicate::Neq(l, r)
            | AbstractPredicate::Lt(l, r)
            | AbstractPredicate::Lte(l, r)
            | AbstractPredicate::Gt(l, r)
            | AbstractPredicate::Gte(l, r)
            | AbstractPredicate::In(l, r)
            | AbstractPredicate::StringLike(l, r, _)
            | AbstractPredicate::StringStartsWith(l, r)
            | AbstractPredicate::StringEndsWith(l, r)
            | AbstractPredicate::JsonContains(l, r)
            | AbstractPredicate::JsonContainedBy(l, r)
            | AbstractPredicate::JsonMatchKey(l, r)
            | AbstractPredicate::JsonMatchAnyKey(l, r)
            | AbstractPredicate::JsonMatchAllKeys(l, r) => vec![l, r],
            AbstractPredicate::And(l, r) | AbstractPredicate::Or(l, r) => {
                let mut result = l.column_paths();
                result.extend(r.column_paths());
                result
            }
            AbstractPredicate::Not(p) => p.column_paths(),
        }
    }

    pub fn predicate(&self) -> Predicate<'a> {
        match self {
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

            AbstractPredicate::And(l, r) => Predicate::and(l.predicate(), r.predicate()),
            AbstractPredicate::Or(l, r) => Predicate::or(l.predicate(), r.predicate()),
            AbstractPredicate::Not(p) => Predicate::Not(Box::new(p.predicate())),
        }
    }

    pub fn predicate_x(&'a self, select_transformer: &impl SelectTransformer) -> Predicate<'a> {
        fn binary_operator<'a>(
            left: &'a ColumnPath<'a>,
            right: &'a ColumnPath<'a>,
            abstract_predicate_op: fn(
                MaybeOwned<'a, ColumnPath<'a>>,
                MaybeOwned<'a, ColumnPath<'a>>,
            ) -> AbstractPredicate<'a>,
            predicate_op: fn(
                MaybeOwned<'a, Column<'a>>,
                MaybeOwned<'a, Column<'a>>,
            ) -> Predicate<'a>,
            select_transformer: &impl SelectTransformer,
        ) -> Predicate<'a> {
            match components(left) {
                Some((in_left_column, table, foreign_column, tail_links)) => {
                    let right_abstract_select = AbstractSelect {
                        table,
                        selection: Selection::Seq(vec![ColumnSelection {
                            column: SelectionElement::Physical(foreign_column),
                            alias: foreign_column.column_name.clone(),
                        }]),
                        predicate: abstract_predicate_op(
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
                        SelectionLevel::Nested,
                    );

                    let right_select_column = Column::SelectionTableWrapper(Box::new(right_select));

                    // "concerts"."venue_id" in (select "venues"."id" from "venues" where "venues"."name" = $1)
                    Predicate::In(
                        Column::Physical(in_left_column).into(),
                        right_select_column.into(),
                    )
                }
                None => predicate_op(leaf_column(left).into(), leaf_column(right).into()),
            }
        }

        match self {
            AbstractPredicate::True => Predicate::True,
            AbstractPredicate::False => Predicate::False,
            AbstractPredicate::Eq(l, r) => binary_operator(
                l,
                r,
                AbstractPredicate::eq,
                Predicate::eq,
                select_transformer,
            ),
            AbstractPredicate::Neq(l, r) => binary_operator(
                l,
                r,
                AbstractPredicate::neq,
                Predicate::neq,
                select_transformer,
            ),
            _ => todo!(),
        }
    }
}

fn leaf_column<'c>(column_path: &ColumnPath<'c>) -> Column<'c> {
    match column_path {
        ColumnPath::Physical(links) => Column::Physical(links.last().unwrap().self_column.0),
        ColumnPath::Literal(l) => Column::Literal(MaybeOwned::Owned(l.as_ref().clone())),
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
