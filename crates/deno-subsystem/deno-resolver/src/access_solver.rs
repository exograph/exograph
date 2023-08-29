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
            reduce_common_primitive_expression, AccessPredicate, AccessSolver, AccessSolverError,
        },
        context::RequestContext,
        value::Val,
    },
};

use deno_model::{access::ModuleAccessPrimitiveExpression, subsystem::DenoSubsystem};

use crate::module_access_predicate::ModuleAccessPredicate;

// Only to get around the orphan rule while implementing AccessSolver
#[derive(Debug)]
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
        &self,
        request_context: &RequestContext<'a>,
        _input_context: Option<&'a Val>,
        op: &AccessRelationalOp<ModuleAccessPrimitiveExpression>,
    ) -> Result<Option<ModuleAccessPredicateWrapper>, AccessSolverError> {
        async fn reduce_primitive_expression<'a>(
            solver: &DenoSubsystem,
            request_context: &'a RequestContext<'a>,
            expr: &'a ModuleAccessPrimitiveExpression,
        ) -> Result<Option<Val>, AccessSolverError> {
            Ok(match expr {
                ModuleAccessPrimitiveExpression::Common(common_expr) => {
                    reduce_common_primitive_expression(solver, request_context, common_expr).await?
                }
            })
        }

        let (left, right) = op.sides();
        let left_value = reduce_primitive_expression(self, request_context, left).await?;
        let right_value = reduce_primitive_expression(self, request_context, right).await?;

        Ok(match (left_value, right_value) {
            (None, _) | (_, None) => None,
            (Some(ref left_value), Some(ref right_value)) => Some(ModuleAccessPredicateWrapper(
                match op {
                    AccessRelationalOp::Eq(..) => eq_values(left_value, right_value),
                    AccessRelationalOp::Neq(_, _) => neq_values(left_value, right_value),
                    AccessRelationalOp::Lt(_, _) => lt_values(left_value, right_value),
                    AccessRelationalOp::Lte(_, _) => lte_values(left_value, right_value),
                    AccessRelationalOp::Gt(_, _) => gt_values(left_value, right_value),
                    AccessRelationalOp::Gte(_, _) => gte_values(left_value, right_value),
                    AccessRelationalOp::In(..) => in_values(left_value, right_value),
                }
                .into(),
            )),
        })
    }
}
