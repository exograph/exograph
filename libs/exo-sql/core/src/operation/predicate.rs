// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::VectorDistanceFunction;

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

/// A predicate is a boolean expression that can be used in a WHERE clause.
#[derive(Debug, PartialEq, Clone)]
pub enum Predicate<C>
where
    C: PartialEq + ParamEquality,
{
    True,
    False,
    Eq(C, C),
    Neq(C, C),
    Lt(C, C),
    Lte(C, C),
    Gt(C, C),
    Gte(C, C),
    In(C, C),

    // string predicates
    StringLike(C, C, CaseSensitivity),
    StringStartsWith(C, C),
    StringEndsWith(C, C),

    // json predicates
    JsonContains(C, C),
    JsonContainedBy(C, C),
    JsonMatchKey(C, C),
    JsonMatchAnyKey(C, C),
    JsonMatchAllKeys(C, C),

    VectorDistance(C, C, VectorDistanceFunction, NumericComparator, C),

    // Prefer Predicate::and(), which simplifies the clause
    And(Box<Predicate<C>>, Box<Predicate<C>>),
    // Prefer Predicate::or(), which simplifies the clause
    Or(Box<Predicate<C>>, Box<Predicate<C>>),
    // Prefer Predicate::not(), which simplifies the clause
    Not(Box<Predicate<C>>),
}

impl<C> Predicate<C>
where
    C: PartialEq + ParamEquality,
{
    /// Compare two columns and reduce to a simpler predicate if possible.
    pub fn eq(lhs: C, rhs: C) -> Predicate<C> {
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
    pub fn neq(lhs: C, rhs: C) -> Predicate<C> {
        !Self::eq(lhs, rhs)
    }

    /// Logical and of two predicates, reducing to a simpler predicate if possible.
    pub fn and(lhs: Predicate<C>, rhs: Predicate<C>) -> Predicate<C> {
        match (lhs, rhs) {
            (Predicate::False, _) | (_, Predicate::False) => Predicate::False,
            (Predicate::True, rhs) => rhs,
            (lhs, Predicate::True) => lhs,
            (lhs, rhs) if lhs == rhs => lhs,
            (lhs, rhs) => Predicate::And(Box::new(lhs), Box::new(rhs)),
        }
    }

    pub fn is_true(&self) -> bool {
        matches!(self, Predicate::True)
    }

    pub fn is_false(&self) -> bool {
        matches!(self, Predicate::False)
    }

    /// Logical or of two predicates, reducing to a simpler predicate if possible.
    pub fn or(lhs: Predicate<C>, rhs: Predicate<C>) -> Predicate<C> {
        match (lhs, rhs) {
            (Predicate::True, _) | (_, Predicate::True) => Predicate::True,
            (Predicate::False, rhs) => rhs,
            (lhs, Predicate::False) => lhs,
            (lhs, rhs) if lhs == rhs => lhs,
            (lhs, rhs) => Predicate::Or(Box::new(lhs), Box::new(rhs)),
        }
    }
}

impl<C> From<bool> for Predicate<C>
where
    C: PartialEq + ParamEquality,
{
    fn from(b: bool) -> Predicate<C> {
        if b { Predicate::True } else { Predicate::False }
    }
}

impl<C> std::ops::Not for Predicate<C>
where
    C: PartialEq + ParamEquality,
{
    type Output = Predicate<C>;

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
