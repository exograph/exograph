// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::Column;
use super::DatabaseExtension;
use super::ParamEquality;

/// Case sensitivity for string predicates.
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum CaseSensitivity {
    Sensitive,
    Insensitive,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum NumericComparator {
    Eq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
}

/// A predicate over concrete SQL columns: a boolean expression that can be used in a WHERE clause.
#[derive(Debug, PartialEq, Clone)]
pub enum Predicate<Ext: DatabaseExtension> {
    True,
    False,
    Eq(Column<Ext>, Column<Ext>),
    Neq(Column<Ext>, Column<Ext>),
    Lt(Column<Ext>, Column<Ext>),
    Lte(Column<Ext>, Column<Ext>),
    Gt(Column<Ext>, Column<Ext>),
    Gte(Column<Ext>, Column<Ext>),
    In(Column<Ext>, Column<Ext>),

    // string predicates
    StringLike(Column<Ext>, Column<Ext>, CaseSensitivity),
    StringStartsWith(Column<Ext>, Column<Ext>),
    StringEndsWith(Column<Ext>, Column<Ext>),

    // json predicates
    JsonContains(Column<Ext>, Column<Ext>),
    JsonContainedBy(Column<Ext>, Column<Ext>),
    JsonMatchKey(Column<Ext>, Column<Ext>),
    JsonMatchAnyKey(Column<Ext>, Column<Ext>),
    JsonMatchAllKeys(Column<Ext>, Column<Ext>),

    /// Database-specific predicate extension (e.g., pgvector distance predicates)
    Extension(Ext::PredicateExtension<Column<Ext>>),

    // Prefer Predicate::and(), which simplifies the clause
    And(Box<Predicate<Ext>>, Box<Predicate<Ext>>),
    // Prefer Predicate::or(), which simplifies the clause
    Or(Box<Predicate<Ext>>, Box<Predicate<Ext>>),
    // Prefer Predicate::not(), which simplifies the clause
    Not(Box<Predicate<Ext>>),
}

impl<Ext: DatabaseExtension> Predicate<Ext> {
    /// Compare two columns and reduce to a simpler predicate if possible.
    pub fn eq(lhs: Column<Ext>, rhs: Column<Ext>) -> Predicate<Ext> {
        if lhs == rhs {
            Predicate::True
        } else {
            // For literal columns, we can check for Predicate::False directly
            match lhs.param_eq(&rhs) {
                Some(false) => Predicate::False, // We don't need to check for `Some(true)`, since the above `lhs == rhs` check would have taken care of that
                _ => Predicate::Eq(lhs, rhs),
            }
        }
    }

    /// Compare two columns and reduce to a simpler predicate if possible
    pub fn neq(lhs: Column<Ext>, rhs: Column<Ext>) -> Predicate<Ext> {
        !Self::eq(lhs, rhs)
    }

    /// Logical and of two predicates, reducing to a simpler predicate if possible.
    pub fn and(lhs: Predicate<Ext>, rhs: Predicate<Ext>) -> Predicate<Ext> {
        match (lhs, rhs) {
            (Predicate::False, _) | (_, Predicate::False) => Predicate::False,
            (Predicate::True, rhs) => rhs,
            (lhs, Predicate::True) => lhs,
            (lhs, rhs) if lhs == rhs => lhs,
            (lhs, rhs) => Predicate::And(Box::new(lhs), Box::new(rhs)),
        }
    }

    /// Logical or of two predicates, reducing to a simpler predicate if possible.
    pub fn or(lhs: Predicate<Ext>, rhs: Predicate<Ext>) -> Predicate<Ext> {
        match (lhs, rhs) {
            (Predicate::True, _) | (_, Predicate::True) => Predicate::True,
            (Predicate::False, rhs) => rhs,
            (lhs, Predicate::False) => lhs,
            (lhs, rhs) if lhs == rhs => lhs,
            (lhs, rhs) => Predicate::Or(Box::new(lhs), Box::new(rhs)),
        }
    }
}

impl<Ext: DatabaseExtension> std::ops::Not for Predicate<Ext> {
    type Output = Predicate<Ext>;

    fn not(self) -> Self::Output {
        match self {
            // Reduced to a simpler form when possible, else fall back to Predicate::Not
            Predicate::True => Predicate::False,
            Predicate::False => Predicate::True,
            Predicate::Eq(lhs, rhs) => Predicate::Neq(lhs, rhs),
            Predicate::Neq(lhs, rhs) => Predicate::Eq(lhs, rhs),
            Predicate::Lt(lhs, rhs) => Predicate::Gte(lhs, rhs),
            Predicate::Lte(lhs, rhs) => Predicate::Gt(lhs, rhs),
            Predicate::Gt(lhs, rhs) => Predicate::Lte(lhs, rhs),
            Predicate::Gte(lhs, rhs) => Predicate::Lt(lhs, rhs),
            predicate => Predicate::Not(Box::new(predicate)),
        }
    }
}
