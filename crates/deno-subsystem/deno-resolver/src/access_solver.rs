// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! [`AccessSolver`] for the Deno subsystem.

use async_trait::async_trait;

use core_plugin_interface::{
    core_model::access::AccessRelationalOp,
    core_resolver::{
        access_solver::{
            reduce_common_primitive_expression, AccessPredicate, AccessSolver,
            SolvedCommonPrimitiveExpression,
        },
        context::RequestContext,
        value::Val,
    },
};

use deno_model::{access::ModuleAccessPrimitiveExpression, subsystem::DenoSubsystem};

use crate::module_access_predicate::ModuleAccessPredicate;

// Only to get around the orphan rule while implementing AccessSolver
pub struct ModuleAccessPredicateWrapper(pub ModuleAccessPredicate);

impl std::ops::Not for ModuleAccessPredicateWrapper {
    type Output = Self;

    fn not(self) -> Self::Output {
        ModuleAccessPredicateWrapper(self.0.not())
    }
}

impl From<bool> for ModuleAccessPredicateWrapper {
    fn from(value: bool) -> Self {
        ModuleAccessPredicateWrapper(ModuleAccessPredicate::from(value))
    }
}

impl<'a> AccessPredicate<'a> for ModuleAccessPredicateWrapper {
    fn and(self, other: Self) -> Self {
        ModuleAccessPredicateWrapper((self.0.into() && other.0.into()).into())
    }

    fn or(self, other: Self) -> Self {
        ModuleAccessPredicateWrapper((self.0.into() || other.0.into()).into())
    }
}

#[async_trait]
impl<'a> AccessSolver<'a, ModuleAccessPrimitiveExpression, ModuleAccessPredicateWrapper>
    for DenoSubsystem
{
    async fn solve_relational_op(
        &'a self,
        request_context: &'a RequestContext<'a>,
        _input_context: Option<&'a Val>,
        op: &'a AccessRelationalOp<ModuleAccessPrimitiveExpression>,
    ) -> ModuleAccessPredicateWrapper {
        async fn reduce_primitive_expression<'a>(
            solver: &DenoSubsystem,
            request_context: &'a RequestContext<'a>,
            expr: &'a ModuleAccessPrimitiveExpression,
        ) -> SolvedCommonPrimitiveExpression<'a> {
            match expr {
                ModuleAccessPrimitiveExpression::Common(common_expr) => {
                    reduce_common_primitive_expression(solver, request_context, common_expr).await
                }
            }
        }

        let (left, right) = op.sides();
        let left = reduce_primitive_expression(self, request_context, left).await;
        let right = reduce_primitive_expression(self, request_context, right).await;

        /// Compare two JSON values
        type ValuePredicateFn<'a> = fn(Val, Val) -> ModuleAccessPredicate;

        // A helper to reduce code duplication in the match below
        let helper = |unresolved_context_predicate: ModuleAccessPredicate,
                      value_predicate: ValuePredicateFn<'a>|
         -> ModuleAccessPredicate {
            match (left, right) {
                (SolvedCommonPrimitiveExpression::UnresolvedContext(_), _)
                | (_, SolvedCommonPrimitiveExpression::UnresolvedContext(_)) => {
                    unresolved_context_predicate
                }
                (
                    SolvedCommonPrimitiveExpression::Value(left_value),
                    SolvedCommonPrimitiveExpression::Value(right_value),
                ) => value_predicate(left_value, right_value),
            }
        };

        // Currently, we don't support expressions such as <, >, <=, >=, but we can easily add them later
        ModuleAccessPredicateWrapper(match op {
            AccessRelationalOp::Eq(..) => helper(ModuleAccessPredicate::False, |val1, val2| {
                (val1 == val2).into()
            }),
            AccessRelationalOp::Neq(_, _) => helper(
                ModuleAccessPredicate::True, // If a context is undefined, declare the expression as a match. For example, `AuthContext.role != "ADMIN"` for anonymous user evaluates to true
                |val1, val2| (val1 != val2).into(),
            ),
            AccessRelationalOp::In(..) => helper(
                ModuleAccessPredicate::False,
                |left_value, right_value| match right_value {
                    Val::List(values) => values.contains(&left_value).into(),
                    _ => unreachable!("The right side operand of `in` operator must be an array"), // This never happens see relational_op::in_relation_match
                },
            ),
            _ => todo!("Unsupported relational operator"),
        })
    }
}
