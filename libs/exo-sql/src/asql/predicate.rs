// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::sql::predicate::Predicate;

use super::column_path::ColumnPath;

/// A version of predicate that uses `ColumnPath`s so that resolvers don't have to deal with
/// low-level concepts such as joins and subselects. These are handled by the
/// `transformer::*_transformer` modules.
pub type AbstractPredicate<'a> = Predicate<ColumnPath<'a>>;

impl<'a> AbstractPredicate<'a> {
    /// Compute the set of column paths that are referenced by this predicate.
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
}
