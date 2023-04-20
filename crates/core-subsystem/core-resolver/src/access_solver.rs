// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_trait::async_trait;
use core_model::access::{AccessLogicalExpression, AccessPredicateExpression, AccessRelationalOp};

use crate::request_context::RequestContext;

/// Access predicate that can be logically combined with other predicates.
pub trait AccessPredicate<'a>:
    From<bool> + std::ops::Not<Output = Self> + 'a + Send + Sync
{
    fn and(self, other: Self) -> Self;
    fn or(self, other: Self) -> Self;
}

/// Solve access control logic.
///
/// Typically, the user of this trait will use the `solve` method.
///
/// ## Parameters:
/// - `PrimExpr`: Primitive expression type
/// - `Res`: Result predicate type
#[async_trait]
pub trait AccessSolver<'a, PrimExpr, Res>
where
    PrimExpr: Send + Sync,
    Res: AccessPredicate<'a>,
{
    /// Solve access control logic.
    ///
    /// Typically, this method (through the implementation of `and`, `or`, `not` as well as
    /// `solve_relational_op`) tries to produce the simplest possible predicate given the request
    /// context. For example, `AuthContext.id == 1` will produce true or false depending on the
    /// value of `AuthContext.id` in the request context. However, `AuthContext.id == 1 &&
    /// self.published` might produce a residue `self.published` if the `AuthContext.id` is 1. This
    /// scheme allows the implementor to optimize to avoid passing a filter to the downstream data
    /// source as well as return a "Not authorized" error when possible (instead of an empty/null
    /// result).
    async fn solve(
        &'a self,
        request_context: &'a RequestContext<'a>,
        expr: &'a AccessPredicateExpression<PrimExpr>,
    ) -> Res {
        match expr {
            AccessPredicateExpression::LogicalOp(op) => {
                self.solve_logical_op(request_context, op).await
            }
            AccessPredicateExpression::RelationalOp(op) => {
                self.solve_relational_op(request_context, op).await
            }
            AccessPredicateExpression::BooleanLiteral(value) => (*value).into(),
        }
    }

    /// Solve relational operation such as `=`, `!=`, `<`, `>`, `<=`, `>=`.
    ///
    /// Since relating two primitive expressions depend on the subsystem, this method is abstract.
    /// For example, a database subsystem produce a relational expression comparing two columns
    /// such as `column_a < column_b`.
    async fn solve_relational_op(
        &'a self,
        request_context: &'a RequestContext<'a>,
        op: &'a AccessRelationalOp<PrimExpr>,
    ) -> Res;

    /// Solve logical operations such as `not`, `and`, `or`.
    async fn solve_logical_op(
        &'a self,
        request_context: &'a RequestContext<'a>,
        op: &'a AccessLogicalExpression<PrimExpr>,
    ) -> Res {
        match op {
            AccessLogicalExpression::Not(underlying) => {
                let underlying_predicate = self.solve(request_context, underlying).await;
                underlying_predicate.not()
            }
            AccessLogicalExpression::And(left, right) => {
                let left_predicate = self.solve(request_context, left).await;
                let right_predicate = self.solve(request_context, right).await;

                left_predicate.and(right_predicate)
            }
            AccessLogicalExpression::Or(left, right) => {
                let left_predicate = self.solve(request_context, left).await;
                let right_predicate = self.solve(request_context, right).await;

                left_predicate.or(right_predicate)
            }
        }
    }
}
