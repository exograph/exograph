// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Module-level access checking shared across all resolvers (GraphQL, RPC, etc.).

use async_trait::async_trait;

use common::context::RequestContext;
use common::value::Val;
use core_model::access::AccessRelationalOp;
use core_resolver::access_solver::{
    AccessInput, AccessPredicate, AccessSolution, AccessSolver, AccessSolverError, eq_values,
    gt_values, gte_values, in_values, lt_values, lte_values, neq_values,
    reduce_common_primitive_expression,
};

use subsystem_model_util::access::ModuleAccessPrimitiveExpression;
use subsystem_model_util::module::ModuleMethod;
use subsystem_model_util::subsystem::ModuleSubsystem;
use subsystem_model_util::types::{ModuleCompositeType, ModuleOperationReturnType, ModuleTypeKind};

pub use crate::module_access_predicate::ModuleAccessPredicate;

/// Shared implementation of `solve_relational_op` for any type implementing `ContextContainer`.
///
/// This is used by both the `AccessSolver for ModuleSubsystem` impl here and
/// the `AccessSolver for DenoSubsystem` impl in `deno-graphql-resolver`.
pub async fn solve_module_relational_op<'a>(
    solver: &(impl core_model::context_type::ContextContainer + Sync + Send),
    request_context: &RequestContext<'a>,
    op: &AccessRelationalOp<ModuleAccessPrimitiveExpression>,
) -> Result<AccessSolution<ModuleAccessPredicate>, AccessSolverError> {
    async fn reduce_primitive_expression<'a>(
        solver: &(impl core_model::context_type::ContextContainer + Sync + Send),
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
    let left_value = reduce_primitive_expression(solver, request_context, left).await?;
    let right_value = reduce_primitive_expression(solver, request_context, right).await?;

    Ok(match (left_value, right_value) {
        (None, _) | (_, None) => AccessSolution::Unsolvable(ModuleAccessPredicate::False),
        (Some(ref left_value), Some(ref right_value)) => AccessSolution::Solved(
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
        ),
    })
}

#[async_trait]
impl<'a> AccessSolver<'a, ModuleAccessPrimitiveExpression, ModuleAccessPredicate>
    for ModuleSubsystem
{
    async fn solve_relational_op(
        &self,
        request_context: &RequestContext<'a>,
        _input_value: Option<&AccessInput<'a>>,
        op: &AccessRelationalOp<ModuleAccessPrimitiveExpression>,
    ) -> Result<AccessSolution<ModuleAccessPredicate>, AccessSolverError> {
        solve_module_relational_op(self, request_context, op).await
    }
}

/// Check module-level access for a method call.
///
/// Evaluates both type-level and method-level access predicates.
/// Returns `true` if the call is allowed, `false` otherwise.
pub async fn check_module_access(
    method: &ModuleMethod,
    subsystem: &ModuleSubsystem,
    request_context: &RequestContext<'_>,
) -> Result<bool, AccessSolverError> {
    match &method.return_type {
        ModuleOperationReturnType::Own(return_type) => {
            let return_type = return_type.typ(&subsystem.module_types);

            let type_level_access = match &return_type.kind {
                ModuleTypeKind::Primitive | ModuleTypeKind::Injected => true,
                ModuleTypeKind::Composite(ModuleCompositeType { access, .. }) => subsystem
                    .solve(request_context, None, &access.value)
                    .await?
                    .map(|r| r.is_true())
                    .resolve(),
            };

            let method_level_access: ModuleAccessPredicate = subsystem
                .solve(request_context, None, &method.access.value)
                .await?
                .resolve();

            // deny if either access check fails
            Ok(type_level_access && !method_level_access.is_false())
        }
        ModuleOperationReturnType::Foreign(_) => {
            // For foreign types, the module doesn't impose its own access control.
            // The associated code may impose any required access control and in the
            // typical case of using Exograph.executeQuery(), that itself will apply
            // the necessary access control.
            Ok(true)
        }
    }
}
