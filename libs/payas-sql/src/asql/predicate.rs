use maybe_owned::MaybeOwned;

use crate::{
    asql::select::SelectionLevel,
    sql::{column::Column, predicate::Predicate},
    transform::transformer::SelectTransformer,
    AbstractSelect, ColumnSelection, Selection, SelectionElement,
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

    pub fn predicate(&'a self) -> Predicate<'a> {
        fn leaf_column<'c>(
            column_path: MaybeOwned<'c, ColumnPath<'c>>,
        ) -> MaybeOwned<'c, Column<'c>> {
            match column_path {
                MaybeOwned::Borrowed(ColumnPath::Physical(links)) => {
                    Column::Physical(links.last().unwrap().self_column.0).into()
                }
                MaybeOwned::Owned(ColumnPath::Physical(links)) => {
                    Column::Physical(links.last().unwrap().self_column.0).into()
                }
                MaybeOwned::Owned(ColumnPath::Literal(l)) => Column::Literal(l).into(),
                MaybeOwned::Borrowed(ColumnPath::Literal(l)) => {
                    Column::Literal(MaybeOwned::Borrowed(l)).into()
                }
                MaybeOwned::Owned(ColumnPath::Null) | MaybeOwned::Borrowed(&ColumnPath::Null) => {
                    panic!("Unexpected column path null")
                }
            }
        }

        match self {
            AbstractPredicate::True => Predicate::True,
            AbstractPredicate::False => Predicate::False,
            AbstractPredicate::Eq(l, r) => Predicate::eq(
                leaf_column(MaybeOwned::Borrowed(l)),
                leaf_column(MaybeOwned::Borrowed(r)),
            ),
            AbstractPredicate::Neq(l, r) => Predicate::neq(
                leaf_column(MaybeOwned::Borrowed(l)),
                leaf_column(MaybeOwned::Borrowed(r)),
            ),
            AbstractPredicate::Lt(l, r) => Predicate::Lt(
                leaf_column(MaybeOwned::Borrowed(l)),
                leaf_column(MaybeOwned::Borrowed(r)),
            ),
            AbstractPredicate::Lte(l, r) => Predicate::Lte(
                leaf_column(MaybeOwned::Borrowed(l)),
                leaf_column(MaybeOwned::Borrowed(r)),
            ),
            AbstractPredicate::Gt(l, r) => Predicate::Gt(
                leaf_column(MaybeOwned::Borrowed(l)),
                leaf_column(MaybeOwned::Borrowed(r)),
            ),
            AbstractPredicate::Gte(l, r) => Predicate::Gte(
                leaf_column(MaybeOwned::Borrowed(l)),
                leaf_column(MaybeOwned::Borrowed(r)),
            ),
            AbstractPredicate::In(l, r) => Predicate::In(
                leaf_column(MaybeOwned::Borrowed(l)),
                leaf_column(MaybeOwned::Borrowed(r)),
            ),

            AbstractPredicate::StringLike(l, r, cs) => Predicate::StringLike(
                leaf_column(MaybeOwned::Borrowed(l)),
                leaf_column(MaybeOwned::Borrowed(r)),
                *cs,
            ),
            AbstractPredicate::StringStartsWith(l, r) => Predicate::StringStartsWith(
                leaf_column(MaybeOwned::Borrowed(l)),
                leaf_column(MaybeOwned::Borrowed(r)),
            ),
            AbstractPredicate::StringEndsWith(l, r) => Predicate::StringEndsWith(
                leaf_column(MaybeOwned::Borrowed(l)),
                leaf_column(MaybeOwned::Borrowed(r)),
            ),

            AbstractPredicate::JsonContains(l, r) => Predicate::JsonContains(
                leaf_column(MaybeOwned::Borrowed(l)),
                leaf_column(MaybeOwned::Borrowed(r)),
            ),
            AbstractPredicate::JsonContainedBy(l, r) => Predicate::JsonContainedBy(
                leaf_column(MaybeOwned::Borrowed(l)),
                leaf_column(MaybeOwned::Borrowed(r)),
            ),
            AbstractPredicate::JsonMatchKey(l, r) => Predicate::JsonMatchKey(
                leaf_column(MaybeOwned::Borrowed(l)),
                leaf_column(MaybeOwned::Borrowed(r)),
            ),
            AbstractPredicate::JsonMatchAnyKey(l, r) => Predicate::JsonMatchAnyKey(
                leaf_column(MaybeOwned::Borrowed(l)),
                leaf_column(MaybeOwned::Borrowed(r)),
            ),
            AbstractPredicate::JsonMatchAllKeys(l, r) => Predicate::JsonMatchAllKeys(
                leaf_column(MaybeOwned::Borrowed(l)),
                leaf_column(MaybeOwned::Borrowed(r)),
            ),

            AbstractPredicate::And(l, r) => Predicate::and(l.predicate(), r.predicate()),
            AbstractPredicate::Or(l, r) => Predicate::or(l.predicate(), r.predicate()),
            AbstractPredicate::Not(p) => Predicate::Not(Box::new(p.predicate())),
        }
    }

    pub fn predicate_x(&'a self, select_transformer: &impl SelectTransformer) -> Predicate<'a> {
        match self {
            AbstractPredicate::Eq(l, r) => {
                // "concerts"."venue_id" in (select "venues"."id" from "venues" where "venues"."name" = $1)

                println!("predicate_x:\n{l:#?}\n{r:#?}");

                let (in_left_column, table, foreign_column, tail_links) = match l.as_ref() {
                    ColumnPath::Physical(links) => {
                        let (head, tail) = links.split_first().unwrap();

                        (
                            Column::Physical(head.self_column.0),
                            head.self_column.1,
                            head.linked_column.unwrap().0,
                            tail,
                        )
                    }
                    _ => todo!(),
                };

                let l_abstract_select = AbstractSelect {
                    table,
                    selection: Selection::Seq(vec![ColumnSelection {
                        column: SelectionElement::Physical(foreign_column),
                        alias: foreign_column.column_name.clone(),
                    }]),
                    predicate: AbstractPredicate::Eq(
                        MaybeOwned::Owned(ColumnPath::Physical(tail_links.to_vec())),
                        MaybeOwned::Borrowed(r),
                    ),
                    order_by: None,
                    offset: None,
                    limit: None,
                };

                let l_select =
                    select_transformer.to_select(&l_abstract_select, None, SelectionLevel::Nested);

                let x = Column::SelectionTableWrapper(Box::new(l_select));

                let inx = Predicate::In(in_left_column.into(), x.into());
                println!("in: {inx:#?}");

                inx
            }
            _ => todo!(),
        }
    }
}
