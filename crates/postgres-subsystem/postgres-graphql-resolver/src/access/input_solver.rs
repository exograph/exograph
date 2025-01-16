// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_trait::async_trait;
use common::context::RequestContext;
use common::value::Val;
use core_plugin_interface::{
    core_model::access::AccessRelationalOp,
    core_resolver::access_solver::{
        eq_values, gt_values, gte_values, in_values, lt_values, lte_values, neq_values,
        reduce_common_primitive_expression, AccessSolver, AccessSolverError,
    },
};
use postgres_graphql_model::subsystem::PostgresGraphQLSubsystem;

use postgres_core_model::access::InputAccessPrimitiveExpression;

use super::access_op::AbstractPredicateWrapper;

#[derive(Debug)]
pub enum SolvedJsonPrimitiveExpression {
    Common(Option<Val>),
    Path(Vec<String>),
}

#[async_trait]
impl<'a> AccessSolver<'a, InputAccessPrimitiveExpression, AbstractPredicateWrapper>
    for PostgresGraphQLSubsystem
{
    async fn solve_relational_op(
        &self,
        request_context: &RequestContext<'a>,
        input_context: Option<&'a Val>,
        op: &AccessRelationalOp<InputAccessPrimitiveExpression>,
    ) -> Result<Option<AbstractPredicateWrapper>, AccessSolverError> {
        async fn reduce_primitive_expression<'a>(
            solver: &PostgresGraphQLSubsystem,
            request_context: &'a RequestContext<'a>,
            expr: &'a InputAccessPrimitiveExpression,
        ) -> Result<Option<SolvedJsonPrimitiveExpression>, AccessSolverError> {
            Ok(match expr {
                InputAccessPrimitiveExpression::Common(expr) => {
                    let primitive_expr =
                        reduce_common_primitive_expression(solver, request_context, expr).await?;
                    Some(SolvedJsonPrimitiveExpression::Common(primitive_expr))
                }
                InputAccessPrimitiveExpression::Path(path, _) => {
                    Some(SolvedJsonPrimitiveExpression::Path(path.clone()))
                }
                InputAccessPrimitiveExpression::Function(_, _) => {
                    unreachable!("Function calls should not remain in the resolver expression")
                }
            })
        }

        let (left, right) = op.sides();
        let left = reduce_primitive_expression(self, request_context, left).await?;
        let right = reduce_primitive_expression(self, request_context, right).await?;

        let (left, right) = match (left, right) {
            (Some(left), Some(right)) => (left, right),
            _ => return Ok(None), // If either side is None, we can't produce a predicate
        };

        type ValuePredicateFn = fn(&Val, &Val) -> bool;

        let helper = |value_predicate: ValuePredicateFn| -> Option<bool> {
            match (left, right) {
                (SolvedJsonPrimitiveExpression::Common(None), _)
                | (_, SolvedJsonPrimitiveExpression::Common(None)) => None,

                (
                    SolvedJsonPrimitiveExpression::Path(left_path),
                    SolvedJsonPrimitiveExpression::Path(right_path),
                ) => Some(match_paths(
                    &left_path,
                    &right_path,
                    input_context,
                    value_predicate,
                )),

                (
                    SolvedJsonPrimitiveExpression::Common(Some(left_value)),
                    SolvedJsonPrimitiveExpression::Common(Some(right_value)),
                ) => Some(value_predicate(&left_value, &right_value)),

                // The next two need to be handled separately, since we need to pass the left side
                // and right side to the predicate in the correct order. For example, `age > 18` is
                // different from `18 > age`.
                (
                    SolvedJsonPrimitiveExpression::Common(Some(left_value)),
                    SolvedJsonPrimitiveExpression::Path(right_path),
                ) => {
                    let right_value = resolve_value(input_context.unwrap(), &right_path);
                    // If the user didn't provide a value, we evaluate to true. Since the purpose of
                    // an input predicate is to enforce an invariant, if the user didn't provide a
                    // value, the original value will remain unchanged thus keeping the invariant
                    // intact.
                    match right_value {
                        Some(right_value) => Some(value_predicate(&left_value, right_value)),
                        None => Some(true),
                    }
                }

                (
                    SolvedJsonPrimitiveExpression::Path(left_path),
                    SolvedJsonPrimitiveExpression::Common(Some(right_value)),
                ) => {
                    let left_value = resolve_value(input_context.unwrap(), &left_path);
                    // See above
                    match left_value {
                        Some(left_value) => Some(value_predicate(left_value, &right_value)),
                        None => Some(true),
                    }
                }
            }
        };

        Ok(match op {
            AccessRelationalOp::Eq(..) => helper(eq_values),
            AccessRelationalOp::Neq(_, _) => helper(neq_values),
            AccessRelationalOp::Lt(_, _) => helper(lt_values),
            AccessRelationalOp::Lte(_, _) => helper(lte_values),
            AccessRelationalOp::Gt(_, _) => helper(gt_values),
            AccessRelationalOp::Gte(_, _) => helper(gte_values),
            AccessRelationalOp::In(..) => helper(in_values),
        }
        .map(|p| AbstractPredicateWrapper(p.into())))
    }
}

fn match_paths<'a>(
    left_path: &'a Vec<String>,
    right_path: &'a Vec<String>,
    input_context: Option<&'a Val>,
    match_values: fn(&Val, &Val) -> bool,
) -> bool {
    let left_value = resolve_value(input_context.unwrap(), left_path).unwrap();
    let right_value = resolve_value(input_context.unwrap(), right_path).unwrap();
    match_values(left_value, right_value)
}

fn resolve_value<'a>(val: &'a Val, path: &'a Vec<String>) -> Option<&'a Val> {
    let mut current = val;
    for part in path {
        match current {
            Val::Object(map) => {
                current = map.get(part)?;
            }
            _ => return None,
        }
    }
    Some(current)
}
