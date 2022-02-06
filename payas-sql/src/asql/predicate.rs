use maybe_owned::MaybeOwned;

use crate::sql::{
    column::Column,
    predicate::{CaseSensitivity, Predicate},
};

use super::column_path::ColumnPath;

// For now a copied-and-modified version of sql::Predicate. The difference is use of ColumnPath instead of Column.

#[derive(Debug)]
pub enum AbstractPredicate<'a> {
    True,
    False,
    Eq(ColumnPath<'a>, ColumnPath<'a>),
    Neq(ColumnPath<'a>, ColumnPath<'a>),
    Lt(ColumnPath<'a>, ColumnPath<'a>),
    Lte(ColumnPath<'a>, ColumnPath<'a>),
    Gt(ColumnPath<'a>, ColumnPath<'a>),
    Gte(ColumnPath<'a>, ColumnPath<'a>),
    In(ColumnPath<'a>, ColumnPath<'a>),

    // string predicates
    StringLike(ColumnPath<'a>, ColumnPath<'a>, CaseSensitivity),
    StringStartsWith(ColumnPath<'a>, ColumnPath<'a>),
    StringEndsWith(ColumnPath<'a>, ColumnPath<'a>),

    // json predicates
    JsonContains(ColumnPath<'a>, ColumnPath<'a>),
    JsonContainedBy(ColumnPath<'a>, ColumnPath<'a>),
    JsonMatchKey(ColumnPath<'a>, ColumnPath<'a>),
    JsonMatchAnyKey(ColumnPath<'a>, ColumnPath<'a>),
    JsonMatchAllKeys(ColumnPath<'a>, ColumnPath<'a>),

    //
    // Prefer Predicate::and(), which simplifies the clause, to construct an And expression
    And(Box<AbstractPredicate<'a>>, Box<AbstractPredicate<'a>>),
    // Prefer Predicate::or(), which simplifies the clause, to construct an Or expression
    Or(Box<AbstractPredicate<'a>>, Box<AbstractPredicate<'a>>),
    Not(Box<AbstractPredicate<'a>>),
}

impl<'a> AbstractPredicate<'a> {
    pub fn column_paths(&'a self) -> Vec<&'a ColumnPath<'a>> {
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
        fn leaf_column<'a>(column_path: &'a ColumnPath<'a>) -> MaybeOwned<'a, Column<'a>> {
            match column_path {
                ColumnPath::Physical(links) => {
                    Column::Physical(links.last().unwrap().self_column.0).into()
                }
                ColumnPath::Literal(l) => Column::Literal(Box::new(5)).into(),
            }
        }

        match self {
            AbstractPredicate::True => Predicate::True,
            AbstractPredicate::False => Predicate::False,
            AbstractPredicate::Eq(l, r) => Predicate::Eq(leaf_column(l), leaf_column(r)),
            AbstractPredicate::Neq(l, r) => Predicate::Neq(leaf_column(l), leaf_column(r)),
            AbstractPredicate::Lt(l, r) => Predicate::Lt(leaf_column(l), leaf_column(r)),
            AbstractPredicate::Lte(l, r) => Predicate::Lte(leaf_column(l), leaf_column(r)),
            AbstractPredicate::Gt(l, r) => Predicate::Gt(leaf_column(l), leaf_column(r)),
            AbstractPredicate::Gte(l, r) => Predicate::Gte(leaf_column(l), leaf_column(r)),
            AbstractPredicate::In(l, r) => Predicate::In(leaf_column(l), leaf_column(r)),

            AbstractPredicate::StringLike(l, r, cs) => {
                Predicate::StringLike(leaf_column(l), leaf_column(r), cs.clone())
            }
            AbstractPredicate::StringStartsWith(l, r) => {
                Predicate::StringStartsWith(leaf_column(l), leaf_column(r))
            }
            AbstractPredicate::StringEndsWith(l, r) => {
                Predicate::StringEndsWith(leaf_column(l), leaf_column(r))
            }

            AbstractPredicate::JsonContains(l, r) => {
                Predicate::JsonContains(leaf_column(l), leaf_column(r))
            }
            AbstractPredicate::JsonContainedBy(l, r) => {
                Predicate::JsonContainedBy(leaf_column(l), leaf_column(r))
            }
            AbstractPredicate::JsonMatchKey(l, r) => {
                Predicate::JsonMatchKey(leaf_column(l), leaf_column(r))
            }
            AbstractPredicate::JsonMatchAnyKey(l, r) => {
                Predicate::JsonMatchAnyKey(leaf_column(l), leaf_column(r))
            }
            AbstractPredicate::JsonMatchAllKeys(l, r) => {
                Predicate::JsonMatchAllKeys(leaf_column(l), leaf_column(r))
            }

            AbstractPredicate::And(l, r) => Predicate::And(
                Box::new(l.predicate().into()),
                Box::new(r.predicate().into()),
            ),
            AbstractPredicate::Or(l, r) => Predicate::Or(
                Box::new(l.predicate().into()),
                Box::new(r.predicate().into()),
            ),
            AbstractPredicate::Not(p) => Predicate::Not(Box::new(p.predicate().into())),
        }
    }
}
