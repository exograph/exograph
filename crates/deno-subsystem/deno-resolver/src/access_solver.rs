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
            eq_values, gt_values, gte_values, in_values, lt_values, lte_values, neq_values,
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
        type ValuePredicateFn = fn(&Val, &Val) -> bool;

        // A helper to reduce code duplication in the match below
        let helper =
            |unresolved_context_predicate: bool, value_predicate: ValuePredicateFn| -> bool {
                match (left, right) {
                    (SolvedCommonPrimitiveExpression::UnresolvedContext(_), _)
                    | (_, SolvedCommonPrimitiveExpression::UnresolvedContext(_)) => {
                        unresolved_context_predicate
                    }
                    (
                        SolvedCommonPrimitiveExpression::Value(left_value),
                        SolvedCommonPrimitiveExpression::Value(right_value),
                    ) => value_predicate(&left_value, &right_value),
                }
            };

        ModuleAccessPredicateWrapper(
            match op {
                AccessRelationalOp::Eq(..) => helper(false, eq_values),
                AccessRelationalOp::Neq(_, _) => helper(
                    // If a context is undefined, declare the expression as a match. For example,
                    // `AuthContext.role != "ADMIN"` for anonymous user evaluates to true
                    true, neq_values,
                ),
                AccessRelationalOp::Lt(_, _) => helper(false, lt_values), // TODO: See issue #611
                AccessRelationalOp::Lte(_, _) => helper(false, lte_values),
                AccessRelationalOp::Gt(_, _) => helper(false, gt_values),
                AccessRelationalOp::Gte(_, _) => helper(false, gte_values),
                AccessRelationalOp::In(..) => helper(false, in_values),
            }
            .into(),
        )
    }
}
