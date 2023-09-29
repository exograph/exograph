// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{sql::predicate::Predicate, ColumnPathLink};

use super::column_path::{ColumnPath, RelationLink};

/// A version of predicate that uses `ColumnPath`s so that resolvers don't have to deal with
/// low-level concepts such as joins and subselects. These are handled by the
/// `transformer::*_transformer` modules.
pub type AbstractPredicate = Predicate<ColumnPath>;

impl AbstractPredicate {
    /// Compute the set of column paths that are referenced by this predicate.
    pub fn column_paths(&self) -> Vec<&ColumnPath> {
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

    pub fn common_relation_link(&self) -> Option<RelationLink> {
        let mut result = None;
        for column_path in self.column_paths() {
            if let ColumnPath::Physical(physical_path) = column_path {
                let head = physical_path.head();
                match head {
                    ColumnPathLink::Relation(link) => {
                        if let Some(existing_link) = &result {
                            if existing_link != link {
                                return None;
                            }
                        } else {
                            result = Some(link.clone());
                        }
                    }
                    ColumnPathLink::Leaf(_) => {
                        return None;
                    }
                }
            }
        }
        result
    }

    pub fn subselect_predicate(&self) -> AbstractPredicate {
        fn binary_operator(
            l: &ColumnPath,
            r: &ColumnPath,
            constructor: impl Fn(ColumnPath, ColumnPath) -> AbstractPredicate,
        ) -> AbstractPredicate {
            let l_tail = match l {
                ColumnPath::Physical(physical_path) => {
                    ColumnPath::Physical(physical_path.tail().unwrap())
                }
                _ => l.clone(),
            };
            let r_tail = match r {
                ColumnPath::Physical(physical_path) => {
                    ColumnPath::Physical(physical_path.tail().unwrap())
                }
                _ => r.clone(),
            };

            constructor(l_tail, r_tail)
        }

        match self {
            AbstractPredicate::True => AbstractPredicate::True,
            AbstractPredicate::False => AbstractPredicate::False,
            AbstractPredicate::Eq(l, r) => binary_operator(l, r, AbstractPredicate::Eq),
            AbstractPredicate::Neq(l, r) => binary_operator(l, r, AbstractPredicate::Neq),
            AbstractPredicate::Lt(l, r) => binary_operator(l, r, AbstractPredicate::Lt),
            AbstractPredicate::Lte(l, r) => binary_operator(l, r, AbstractPredicate::Lte),
            AbstractPredicate::Gt(l, r) => binary_operator(l, r, AbstractPredicate::Gt),
            AbstractPredicate::Gte(l, r) => binary_operator(l, r, AbstractPredicate::Gte),
            AbstractPredicate::In(l, r) => binary_operator(l, r, AbstractPredicate::In),
            AbstractPredicate::StringLike(l, r, sens) => {
                binary_operator(l, r, |l, r| AbstractPredicate::StringLike(l, r, *sens))
            }
            AbstractPredicate::StringStartsWith(l, r) => {
                binary_operator(l, r, AbstractPredicate::StringStartsWith)
            }
            AbstractPredicate::StringEndsWith(l, r) => {
                binary_operator(l, r, AbstractPredicate::StringEndsWith)
            }
            AbstractPredicate::JsonContains(l, r) => {
                binary_operator(l, r, AbstractPredicate::JsonContains)
            }
            AbstractPredicate::JsonContainedBy(l, r) => {
                binary_operator(l, r, AbstractPredicate::JsonContainedBy)
            }
            AbstractPredicate::JsonMatchKey(l, r) => {
                binary_operator(l, r, AbstractPredicate::JsonMatchKey)
            }
            AbstractPredicate::JsonMatchAnyKey(l, r) => {
                binary_operator(l, r, AbstractPredicate::JsonMatchAnyKey)
            }
            AbstractPredicate::JsonMatchAllKeys(l, r) => {
                binary_operator(l, r, AbstractPredicate::JsonMatchAllKeys)
            }
            AbstractPredicate::And(l, r) => AbstractPredicate::And(
                Box::new(l.subselect_predicate()),
                Box::new(r.subselect_predicate()),
            ),
            AbstractPredicate::Or(l, r) => AbstractPredicate::Or(
                Box::new(l.subselect_predicate()),
                Box::new(r.subselect_predicate()),
            ),
            AbstractPredicate::Not(p) => AbstractPredicate::Not(Box::new(p.subselect_predicate())),
        }
    }
}
