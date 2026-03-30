// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! [`AccessSolver`] bridge for [`DenoSubsystem`].
//!
//! The shared access-checking logic lives in `subsystem-resolver-util`.
//! This module provides the orphan-rule-required impl for `DenoSubsystem`,
//! using a local newtype wrapper around the shared predicate.

use async_trait::async_trait;

use common::context::RequestContext;
use core_model::access::AccessRelationalOp;
use core_resolver::access_solver::{
    AccessInput, AccessPredicate, AccessSolution, AccessSolver, AccessSolverError,
};

use deno_graphql_model::subsystem::DenoSubsystem;
use subsystem_model_util::access::ModuleAccessPrimitiveExpression;
use subsystem_resolver_util::access::ModuleAccessPredicate;

// Local newtype to satisfy the orphan rule: AccessSolver is in core-resolver,
// ModuleAccessPredicate is in subsystem-resolver-util, and DenoSubsystem
// is in deno-graphql-model — none are local.
#[derive(Debug)]
pub(crate) struct DenoAccessPredicate(pub(crate) ModuleAccessPredicate);

impl std::ops::Not for DenoAccessPredicate {
    type Output = Self;
    fn not(self) -> Self::Output {
        DenoAccessPredicate(self.0.not())
    }
}

impl From<bool> for DenoAccessPredicate {
    fn from(value: bool) -> Self {
        DenoAccessPredicate(ModuleAccessPredicate::from(value))
    }
}

impl AccessPredicate for DenoAccessPredicate {
    fn and(self, other: Self) -> Self {
        DenoAccessPredicate(self.0.and(other.0))
    }
    fn or(self, other: Self) -> Self {
        DenoAccessPredicate(self.0.or(other.0))
    }
    fn is_true(&self) -> bool {
        self.0.is_true()
    }
    fn is_false(&self) -> bool {
        self.0.is_false()
    }
}

#[async_trait]
impl<'a> AccessSolver<'a, ModuleAccessPrimitiveExpression, DenoAccessPredicate> for DenoSubsystem {
    async fn solve_relational_op(
        &self,
        request_context: &RequestContext<'a>,
        _input_value: Option<&AccessInput<'a>>,
        op: &AccessRelationalOp<ModuleAccessPrimitiveExpression>,
    ) -> Result<AccessSolution<DenoAccessPredicate>, AccessSolverError> {
        let result =
            subsystem_resolver_util::access::solve_module_relational_op(self, request_context, op)
                .await?;

        Ok(match result {
            AccessSolution::Solved(p) => AccessSolution::Solved(DenoAccessPredicate(p)),
            AccessSolution::Unsolvable(p) => AccessSolution::Unsolvable(DenoAccessPredicate(p)),
        })
    }
}
