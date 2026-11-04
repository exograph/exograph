// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql_core::operation::{
    CaseSensitivity, DatabaseExtension, ParamEquality, PredicateExtensionPaths,
};

use crate::column_path::ColumnPath;

/// A predicate over `ColumnPath`s, so that resolvers don't have to deal with low-level concepts
/// such as joins and subselects. These are handled by the `transformer::*_transformer` modules,
/// which produce a predicate over concrete columns ([`exo_sql_core::operation::Predicate`]).
#[derive(Debug, PartialEq, Clone)]
pub enum AbstractPredicate<Ext: DatabaseExtension> {
    True,
    False,
    Eq(ColumnPath<Ext>, ColumnPath<Ext>),
    Neq(ColumnPath<Ext>, ColumnPath<Ext>),
    Lt(ColumnPath<Ext>, ColumnPath<Ext>),
    Lte(ColumnPath<Ext>, ColumnPath<Ext>),
    Gt(ColumnPath<Ext>, ColumnPath<Ext>),
    Gte(ColumnPath<Ext>, ColumnPath<Ext>),
    In(ColumnPath<Ext>, ColumnPath<Ext>),

    // string predicates
    StringLike(ColumnPath<Ext>, ColumnPath<Ext>, CaseSensitivity),
    StringStartsWith(ColumnPath<Ext>, ColumnPath<Ext>),
    StringEndsWith(ColumnPath<Ext>, ColumnPath<Ext>),

    // json predicates
    JsonContains(ColumnPath<Ext>, ColumnPath<Ext>),
    JsonContainedBy(ColumnPath<Ext>, ColumnPath<Ext>),
    JsonMatchKey(ColumnPath<Ext>, ColumnPath<Ext>),
    JsonMatchAnyKey(ColumnPath<Ext>, ColumnPath<Ext>),
    JsonMatchAllKeys(ColumnPath<Ext>, ColumnPath<Ext>),

    /// Database-specific predicate extension (e.g., pgvector distance predicates)
    Extension(Ext::PredicateExtension<ColumnPath<Ext>>),

    // Prefer AbstractPredicate::and(), which simplifies the clause
    And(Box<AbstractPredicate<Ext>>, Box<AbstractPredicate<Ext>>),
    // Prefer AbstractPredicate::or(), which simplifies the clause
    Or(Box<AbstractPredicate<Ext>>, Box<AbstractPredicate<Ext>>),
    // Prefer AbstractPredicate::not(), which simplifies the clause
    Not(Box<AbstractPredicate<Ext>>),
}

impl<Ext: DatabaseExtension> AbstractPredicate<Ext> {
    /// Compare two columns and reduce to a simpler predicate if possible.
    pub fn eq(lhs: ColumnPath<Ext>, rhs: ColumnPath<Ext>) -> AbstractPredicate<Ext> {
        if lhs == rhs {
            AbstractPredicate::True
        } else {
            // For literal columns, we can check for AbstractPredicate::False directly
            match lhs.param_eq(&rhs) {
                Some(false) => AbstractPredicate::False, // We don't need to check for `Some(true)`, since the above `lhs == rhs` check would have taken care of that
                _ => AbstractPredicate::Eq(lhs, rhs),
            }
        }
    }

    /// Compare two columns and reduce to a simpler predicate if possible
    pub fn neq(lhs: ColumnPath<Ext>, rhs: ColumnPath<Ext>) -> AbstractPredicate<Ext> {
        !Self::eq(lhs, rhs)
    }

    /// Logical and of two predicates, reducing to a simpler predicate if possible.
    pub fn and(lhs: AbstractPredicate<Ext>, rhs: AbstractPredicate<Ext>) -> AbstractPredicate<Ext> {
        match (lhs, rhs) {
            (AbstractPredicate::False, _) | (_, AbstractPredicate::False) => {
                AbstractPredicate::False
            }
            (AbstractPredicate::True, rhs) => rhs,
            (lhs, AbstractPredicate::True) => lhs,
            (lhs, rhs) if lhs == rhs => lhs,
            (lhs, rhs) => AbstractPredicate::And(Box::new(lhs), Box::new(rhs)),
        }
    }

    pub fn is_true(&self) -> bool {
        matches!(self, AbstractPredicate::True)
    }

    pub fn is_false(&self) -> bool {
        matches!(self, AbstractPredicate::False)
    }

    /// Logical or of two predicates, reducing to a simpler predicate if possible.
    pub fn or(lhs: AbstractPredicate<Ext>, rhs: AbstractPredicate<Ext>) -> AbstractPredicate<Ext> {
        match (lhs, rhs) {
            (AbstractPredicate::True, _) | (_, AbstractPredicate::True) => AbstractPredicate::True,
            (AbstractPredicate::False, rhs) => rhs,
            (lhs, AbstractPredicate::False) => lhs,
            (lhs, rhs) if lhs == rhs => lhs,
            (lhs, rhs) => AbstractPredicate::Or(Box::new(lhs), Box::new(rhs)),
        }
    }
}

impl<Ext: DatabaseExtension> From<bool> for AbstractPredicate<Ext> {
    fn from(b: bool) -> AbstractPredicate<Ext> {
        if b {
            AbstractPredicate::True
        } else {
            AbstractPredicate::False
        }
    }
}

impl<Ext: DatabaseExtension> std::ops::Not for AbstractPredicate<Ext> {
    type Output = AbstractPredicate<Ext>;

    fn not(self) -> Self::Output {
        match self {
            // Reduced to a simpler form when possible, else fall back to AbstractPredicate::Not
            AbstractPredicate::True => AbstractPredicate::False,
            AbstractPredicate::False => AbstractPredicate::True,
            AbstractPredicate::Eq(lhs, rhs) => AbstractPredicate::Neq(lhs, rhs),
            AbstractPredicate::Neq(lhs, rhs) => AbstractPredicate::Eq(lhs, rhs),
            AbstractPredicate::Lt(lhs, rhs) => AbstractPredicate::Gte(lhs, rhs),
            AbstractPredicate::Lte(lhs, rhs) => AbstractPredicate::Gt(lhs, rhs),
            AbstractPredicate::Gt(lhs, rhs) => AbstractPredicate::Lte(lhs, rhs),
            AbstractPredicate::Gte(lhs, rhs) => AbstractPredicate::Lt(lhs, rhs),
            predicate => AbstractPredicate::Not(Box::new(predicate)),
        }
    }
}

/// Extension methods for `AbstractPredicate`
pub trait AbstractPredicateExt<Ext: DatabaseExtension> {
    /// Compute the set of column paths that are referenced by this predicate.
    fn column_paths(&self) -> Vec<&ColumnPath<Ext>>;
}

impl<Ext: DatabaseExtension> AbstractPredicateExt<Ext> for AbstractPredicate<Ext> {
    fn column_paths(&self) -> Vec<&ColumnPath<Ext>> {
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

            AbstractPredicate::Extension(ext) => ext.column_paths(),

            AbstractPredicate::And(l, r) | AbstractPredicate::Or(l, r) => {
                let mut result = l.column_paths();
                result.extend(r.column_paths());
                result
            }
            AbstractPredicate::Not(p) => p.column_paths(),
        }
    }
}
