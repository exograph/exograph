use maybe_owned::MaybeOwned;

use crate::sql::{
    column::Column,
    predicate::{CaseSensitivity, Predicate},
};

use super::column_path::ColumnPath;

// For now a copied-and-modified version of sql::Predicate. The difference is use of ColumnPath instead of Column.

#[derive(Debug, PartialEq)]
pub enum AbstractPredicate<'a> {
    True,
    False,
    Eq(
        MaybeOwned<'a, ColumnPath<'a>>,
        MaybeOwned<'a, ColumnPath<'a>>,
    ),
    Neq(
        MaybeOwned<'a, ColumnPath<'a>>,
        MaybeOwned<'a, ColumnPath<'a>>,
    ),
    Lt(
        MaybeOwned<'a, ColumnPath<'a>>,
        MaybeOwned<'a, ColumnPath<'a>>,
    ),
    Lte(
        MaybeOwned<'a, ColumnPath<'a>>,
        MaybeOwned<'a, ColumnPath<'a>>,
    ),
    Gt(
        MaybeOwned<'a, ColumnPath<'a>>,
        MaybeOwned<'a, ColumnPath<'a>>,
    ),
    Gte(
        MaybeOwned<'a, ColumnPath<'a>>,
        MaybeOwned<'a, ColumnPath<'a>>,
    ),
    In(
        MaybeOwned<'a, ColumnPath<'a>>,
        MaybeOwned<'a, ColumnPath<'a>>,
    ),

    // string predicates
    StringLike(
        MaybeOwned<'a, ColumnPath<'a>>,
        MaybeOwned<'a, ColumnPath<'a>>,
        CaseSensitivity,
    ),
    StringStartsWith(
        MaybeOwned<'a, ColumnPath<'a>>,
        MaybeOwned<'a, ColumnPath<'a>>,
    ),
    StringEndsWith(
        MaybeOwned<'a, ColumnPath<'a>>,
        MaybeOwned<'a, ColumnPath<'a>>,
    ),

    // json predicates
    JsonContains(
        MaybeOwned<'a, ColumnPath<'a>>,
        MaybeOwned<'a, ColumnPath<'a>>,
    ),
    JsonContainedBy(
        MaybeOwned<'a, ColumnPath<'a>>,
        MaybeOwned<'a, ColumnPath<'a>>,
    ),
    JsonMatchKey(
        MaybeOwned<'a, ColumnPath<'a>>,
        MaybeOwned<'a, ColumnPath<'a>>,
    ),
    JsonMatchAnyKey(
        MaybeOwned<'a, ColumnPath<'a>>,
        MaybeOwned<'a, ColumnPath<'a>>,
    ),
    JsonMatchAllKeys(
        MaybeOwned<'a, ColumnPath<'a>>,
        MaybeOwned<'a, ColumnPath<'a>>,
    ),

    //
    // Prefer Predicate::and(), which simplifies the clause, to construct an And expression
    And(Box<AbstractPredicate<'a>>, Box<AbstractPredicate<'a>>),
    // Prefer Predicate::or(), which simplifies the clause, to construct an Or expression
    Or(Box<AbstractPredicate<'a>>, Box<AbstractPredicate<'a>>),
    Not(Box<AbstractPredicate<'a>>),
}

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

    pub fn predicate(self) -> Predicate<'a> {
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
                MaybeOwned::Borrowed(ColumnPath::Literal(_)) => {
                    panic!("Unexpected borrowed literal. Literal in ColumnPath must be owned")
                }
            }
        }

        match self {
            AbstractPredicate::True => Predicate::True,
            AbstractPredicate::False => Predicate::False,
            AbstractPredicate::Eq(l, r) => Predicate::eq(leaf_column(l), leaf_column(r)),
            AbstractPredicate::Neq(l, r) => Predicate::neq(leaf_column(l), leaf_column(r)),
            AbstractPredicate::Lt(l, r) => Predicate::Lt(leaf_column(l), leaf_column(r)),
            AbstractPredicate::Lte(l, r) => Predicate::Lte(leaf_column(l), leaf_column(r)),
            AbstractPredicate::Gt(l, r) => Predicate::Gt(leaf_column(l), leaf_column(r)),
            AbstractPredicate::Gte(l, r) => Predicate::Gte(leaf_column(l), leaf_column(r)),
            AbstractPredicate::In(l, r) => Predicate::In(leaf_column(l), leaf_column(r)),

            AbstractPredicate::StringLike(l, r, cs) => {
                Predicate::StringLike(leaf_column(l), leaf_column(r), cs)
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

            AbstractPredicate::And(l, r) => Predicate::and(l.predicate(), r.predicate()),
            AbstractPredicate::Or(l, r) => Predicate::or(l.predicate(), r.predicate()),
            AbstractPredicate::Not(p) => Predicate::Not(Box::new(p.predicate().into())),
        }
    }

    pub fn from_name(
        op_name: &str,
        lhs: MaybeOwned<'a, ColumnPath<'a>>,
        rhs: MaybeOwned<'a, ColumnPath<'a>>,
    ) -> AbstractPredicate<'a> {
        match op_name {
            "eq" => AbstractPredicate::Eq(lhs, rhs),
            "neq" => AbstractPredicate::Neq(lhs, rhs),
            "lt" => AbstractPredicate::Lt(lhs, rhs),
            "lte" => AbstractPredicate::Lte(lhs, rhs),
            "gt" => AbstractPredicate::Gt(lhs, rhs),
            "gte" => AbstractPredicate::Gte(lhs, rhs),
            "like" => AbstractPredicate::StringLike(lhs, rhs, CaseSensitivity::Sensitive),
            "ilike" => AbstractPredicate::StringLike(lhs, rhs, CaseSensitivity::Insensitive),
            "startsWith" => AbstractPredicate::StringStartsWith(lhs, rhs),
            "endsWith" => AbstractPredicate::StringEndsWith(lhs, rhs),
            "contains" => AbstractPredicate::JsonContains(lhs, rhs),
            "containedBy" => AbstractPredicate::JsonContainedBy(lhs, rhs),
            "matchKey" => AbstractPredicate::JsonMatchKey(lhs, rhs),
            "matchAnyKey" => AbstractPredicate::JsonMatchAnyKey(lhs, rhs),
            "matchAllKeys" => AbstractPredicate::JsonMatchAllKeys(lhs, rhs),
            _ => todo!(),
        }
    }

    pub fn eq(lhs: MaybeOwned<'a, ColumnPath<'a>>, rhs: MaybeOwned<'a, ColumnPath<'a>>) -> Self {
        if lhs == rhs {
            Self::True
        } else {
            match (lhs.as_ref(), rhs.as_ref()) {
                // For literal columns, we can check for Predicate::False directly
                (ColumnPath::Literal(v1), ColumnPath::Literal(v2)) if v1 != v2 => Self::False,
                _ => Self::Eq(lhs, rhs),
            }
        }
    }

    pub fn neq(lhs: MaybeOwned<'a, ColumnPath<'a>>, rhs: MaybeOwned<'a, ColumnPath<'a>>) -> Self {
        !Self::eq(lhs, rhs)
    }

    pub fn and(lhs: Self, rhs: Self) -> Self {
        match (lhs, rhs) {
            (Self::True, rhs) => rhs,
            (lhs, Self::True) => lhs,
            (Self::False, _) => Self::False,
            (_, Self::False) => Self::False,
            (lhs, rhs) => Self::And(Box::new(lhs), Box::new(rhs)),
        }
    }

    pub fn or(lhs: Self, rhs: Self) -> Self {
        match (lhs, rhs) {
            (Self::True, _) => Self::True,
            (_, Self::True) => Self::True,
            (Self::False, rhs) => rhs,
            (lhs, Self::False) => lhs,
            (lhs, rhs) => Self::Or(Box::new(lhs), Box::new(rhs)),
        }
    }
}

impl From<bool> for AbstractPredicate<'static> {
    fn from(b: bool) -> AbstractPredicate<'static> {
        if b {
            AbstractPredicate::True
        } else {
            AbstractPredicate::False
        }
    }
}

impl<'a> std::ops::Not for AbstractPredicate<'a> {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            // Reduced to a simpler form when possible, else fall back to Predicate::Not
            Self::True => Self::False,
            Self::False => Self::True,
            Self::Eq(lhs, rhs) => Self::Neq(lhs, rhs),
            Self::Neq(lhs, rhs) => Self::Eq(lhs, rhs),
            Self::Lt(lhs, rhs) => Self::Gte(lhs, rhs),
            Self::Lte(lhs, rhs) => Self::Gt(lhs, rhs),
            Self::Gt(lhs, rhs) => Self::Lte(lhs, rhs),
            Self::Gte(lhs, rhs) => Self::Lt(lhs, rhs),
            predicate => Self::Not(Box::new(predicate)),
        }
    }
}
