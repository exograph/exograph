// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! [`AccessSolver`] for the Postgres subsystem.
//!
//! This computes a predicate that can be either a boolean value or a residual expression that can
//! be passed down to the the underlying system (for example, a `where` clause to the database
//! query).
//!
//! This module differs from Deno/Wasm in that it has an additional primitive expression type,
//! `ColumnPath`, which we process into a predicate that we can pass to the database query.

use async_trait::async_trait;
use common::context::RequestContext;
use common::value::Val;

use core_model::access::AccessRelationalOp;
use core_resolver::access_solver::{
    AccessInput, AccessPredicate, AccessSolution, AccessSolver, AccessSolverError, eq_values,
    neq_values, reduce_common_primitive_expression,
};
use exo_sql::{
    AbstractPredicate, ColumnPath, ColumnPathLink, PhysicalColumnPath, SQLParamContainer,
};
use postgres_core_model::{
    access::DatabaseAccessPrimitiveExpression, subsystem::PostgresCoreSubsystem,
};

use crate::cast;

fn normalize_column_path(mut column_path: PhysicalColumnPath) -> PhysicalColumnPath {
    let mut segments = Vec::new();
    let mut remaining = Some(column_path.clone());

    while let Some(current) = remaining {
        let (head, tail) = current.split_head();
        segments.push(head);
        remaining = tail;
    }

    if segments.is_empty() {
        return column_path;
    }

    let last = segments.pop().unwrap();
    let normalized_last = match last {
        ColumnPathLink::Relation(relation) if relation.column_pairs.len() == 1 => {
            ColumnPathLink::Leaf(relation.column_pairs[0].self_column_id)
        }
        other => other,
    };

    let mut iter = segments.into_iter();
    let rebuilt = match iter.next() {
        Some(first) => {
            let mut path = PhysicalColumnPath::init(first);
            for segment in iter {
                path = path.push(segment);
            }
            path.push(normalized_last)
        }
        None => PhysicalColumnPath::init(normalized_last),
    };

    rebuilt
}

// Only to get around the orphan rule while implementing AccessSolver
#[derive(Debug)]
pub struct AbstractPredicateWrapper(pub AbstractPredicate);

impl std::ops::Not for AbstractPredicateWrapper {
    type Output = AbstractPredicateWrapper;

    fn not(self) -> Self::Output {
        Self(self.0.not())
    }
}

impl From<bool> for AbstractPredicateWrapper {
    fn from(value: bool) -> Self {
        Self(AbstractPredicate::from(value))
    }
}

impl AccessPredicate for AbstractPredicateWrapper {
    fn and(self, other: Self) -> Self {
        Self(AbstractPredicate::and(self.0, other.0))
    }

    fn or(self, other: Self) -> Self {
        Self(AbstractPredicate::or(self.0, other.0))
    }

    fn is_true(&self) -> bool {
        self.0.is_true()
    }

    fn is_false(&self) -> bool {
        self.0.is_false()
    }
}

#[derive(Debug)]
pub enum SolvedPrimitiveExpression {
    Common(Option<Val>),
    Column(PhysicalColumnPath),
}

#[async_trait]
impl<'a> AccessSolver<'a, DatabaseAccessPrimitiveExpression, AbstractPredicateWrapper>
    for PostgresCoreSubsystem
{
    async fn solve_relational_op(
        &self,
        request_context: &RequestContext<'a>,
        _input_value: Option<&AccessInput<'a>>,
        op: &AccessRelationalOp<DatabaseAccessPrimitiveExpression>,
    ) -> Result<AccessSolution<AbstractPredicateWrapper>, AccessSolverError> {
        async fn reduce_primitive_expression<'a>(
            solver: &PostgresCoreSubsystem,
            request_context: &'a RequestContext<'a>,
            expr: &'a DatabaseAccessPrimitiveExpression,
        ) -> Result<AccessSolution<SolvedPrimitiveExpression>, AccessSolverError> {
            Ok(match expr {
                DatabaseAccessPrimitiveExpression::Common(expr) => {
                    let primitive_expr =
                        reduce_common_primitive_expression(solver, request_context, expr).await?;
                    AccessSolution::Solved(SolvedPrimitiveExpression::Common(primitive_expr))
                }
                DatabaseAccessPrimitiveExpression::Column(column_path, _) => {
                    AccessSolution::Solved(SolvedPrimitiveExpression::Column(
                        normalize_column_path(column_path.clone()),
                    ))
                }
                DatabaseAccessPrimitiveExpression::Function(_, _) => {
                    // TODO: Fix this through better types
                    unreachable!("Function calls should not remain in the resolver expression")
                }
            })
        }

        let (left, right) = op.sides();
        let left = reduce_primitive_expression(self, request_context, left).await?;
        let right = reduce_primitive_expression(self, request_context, right).await?;

        let (left, right) = match (left, right) {
            (AccessSolution::Solved(left), AccessSolution::Solved(right)) => (left, right),
            _ => {
                return Ok(AccessSolution::Unsolvable(AbstractPredicateWrapper(
                    AbstractPredicate::True,
                )));
            } // If either side is None, we can't produce a predicate
        };

        type ColumnPredicateFn = fn(ColumnPath, ColumnPath) -> AbstractPredicate;
        type ValuePredicateFn = fn(Val, Val) -> AbstractPredicate;

        let helper = |column_predicate: ColumnPredicateFn,
                      value_predicate: ValuePredicateFn|
         -> Result<AccessSolution<AbstractPredicate>, AccessSolverError> {
            match (left, right) {
                (SolvedPrimitiveExpression::Common(None), _)
                | (_, SolvedPrimitiveExpression::Common(None)) => {
                    Ok(AccessSolution::Unsolvable(AbstractPredicate::False))
                }

                (
                    SolvedPrimitiveExpression::Column(left_col),
                    SolvedPrimitiveExpression::Column(right_col),
                ) => Ok(AccessSolution::Solved(column_predicate(
                    to_column_path(&left_col),
                    to_column_path(&right_col),
                ))),

                (
                    SolvedPrimitiveExpression::Common(Some(left_value)),
                    SolvedPrimitiveExpression::Common(Some(right_value)),
                ) => Ok(AccessSolution::Solved(value_predicate(
                    left_value,
                    right_value,
                ))),

                // The next two need to be handled separately, since we need to pass the left side
                // and right side to the predicate in the correct order. For example, `age > 18` is
                // different from `18 > age`.
                (
                    SolvedPrimitiveExpression::Common(Some(value)),
                    SolvedPrimitiveExpression::Column(column),
                ) => {
                    let physical_column = column.leaf_column().get_column(&self.database);

                    Ok(AccessSolution::Solved(column_predicate(
                        cast::literal_column_path(&value, &*physical_column.typ, op.needs_unnest())
                            .map_err(|e| {
                                AccessSolverError::Generic(
                                    format!("Failed to cast literal: '{value:?}': {e}").into(),
                                )
                            })?,
                        to_column_path(&column),
                    )))
                }

                (
                    SolvedPrimitiveExpression::Column(column),
                    SolvedPrimitiveExpression::Common(Some(value)),
                ) => {
                    let physical_column = column.leaf_column().get_column(&self.database);

                    let literal_column_path =
                        cast::literal_column_path(&value, &*physical_column.typ, op.needs_unnest())
                            .map_err(|e| {
                                AccessSolverError::Generic(
                                    format!("Failed to cast literal: '{value:?}': {e}").into(),
                                )
                            })?;

                    Ok(AccessSolution::Solved(column_predicate(
                        to_column_path(&column),
                        literal_column_path,
                    )))
                }
            }
        };

        let access_predicate = match op {
            AccessRelationalOp::Eq(..) => {
                helper(AbstractPredicate::eq, |left_value, right_value| {
                    eq_values(&left_value, &right_value).into()
                })
            }
            AccessRelationalOp::Neq(_, _) => {
                helper(AbstractPredicate::neq, |left_value, right_value| {
                    neq_values(&left_value, &right_value).into()
                })
            }
            // For the next four, we could optimize cases where values are comparable, but
            // for now, we generate a predicate and let the database handle it
            AccessRelationalOp::Lt(_, _) => {
                helper(AbstractPredicate::Lt, |left_value, right_value| {
                    AbstractPredicate::Lt(literal_column(left_value), literal_column(right_value))
                })
            }
            AccessRelationalOp::Lte(_, _) => {
                helper(AbstractPredicate::Lte, |left_value, right_value| {
                    AbstractPredicate::Lte(literal_column(left_value), literal_column(right_value))
                })
            }
            AccessRelationalOp::Gt(_, _) => {
                helper(AbstractPredicate::Gt, |left_value, right_value| {
                    AbstractPredicate::Gt(literal_column(left_value), literal_column(right_value))
                })
            }
            AccessRelationalOp::Gte(_, _) => {
                helper(AbstractPredicate::Gte, |left_value, right_value| {
                    AbstractPredicate::Gte(literal_column(left_value), literal_column(right_value))
                })
            }
            AccessRelationalOp::In(..) => helper(
                AbstractPredicate::In,
                |left_value, right_value| match right_value {
                    Val::List(values) => values.contains(&left_value).into(),
                    Val::Null => false.into(),
                    _ => unreachable!("The right side operand of `in` operator must be an array"), // This never happens see relational_op::in_relation_match
                },
            ),
        }?;

        Ok(access_predicate.map(AbstractPredicateWrapper))
    }
}

pub fn to_column_path(physical_column_path: &PhysicalColumnPath) -> ColumnPath {
    ColumnPath::Physical(physical_column_path.clone())
}

/// Converts a value to a literal column path
pub fn literal_column(value: Val) -> ColumnPath {
    match value {
        Val::Null => ColumnPath::Null,
        Val::Bool(v) => ColumnPath::Param(SQLParamContainer::bool(v)),
        Val::Number(v) => ColumnPath::Param(SQLParamContainer::i32(v.as_i64().unwrap() as i32)), // TODO: Deal with the exact number type
        Val::String(v) => ColumnPath::Param(SQLParamContainer::string(v)),
        Val::List(_) | Val::Object(_) | Val::Binary(_) | Val::Enum(_) => todo!(),
    }
}
