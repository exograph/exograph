// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_trait::async_trait;
use core_model::access::{
    AccessLogicalExpression, AccessPredicateExpression, AccessRelationalOp,
    CommonAccessPrimitiveExpression,
};

use crate::{
    context::RequestContext, context_extractor::ContextExtractor, number_cmp::NumberWrapper,
    value::Val,
};

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
        &self,
        request_context: &RequestContext<'a>,
        input_context: Option<&'a Val>, // User provided context (such as input to a mutation)
        expr: &AccessPredicateExpression<PrimExpr>,
    ) -> Option<Res> {
        match expr {
            AccessPredicateExpression::LogicalOp(op) => {
                self.solve_logical_op(request_context, input_context, op)
                    .await
            }
            AccessPredicateExpression::RelationalOp(op) => {
                self.solve_relational_op(request_context, input_context, op)
                    .await
            }
            AccessPredicateExpression::BooleanLiteral(value) => Some((*value).into()),
        }
    }

    /// Solve relational operation such as `=`, `!=`, `<`, `>`, `<=`, `>=`.
    ///
    /// Since relating two primitive expressions depend on the subsystem, this method is abstract.
    /// For example, a database subsystem produce a relational expression comparing two columns
    /// such as `column_a < column_b`.
    async fn solve_relational_op(
        &self,
        request_context: &RequestContext<'a>,
        input_context: Option<&'a Val>,
        op: &AccessRelationalOp<PrimExpr>,
    ) -> Option<Res>;

    /// Solve logical operations such as `not`, `and`, `or`.
    async fn solve_logical_op(
        &self,
        request_context: &RequestContext<'a>,
        input_context: Option<&'a Val>,
        op: &AccessLogicalExpression<PrimExpr>,
    ) -> Option<Res> {
        match op {
            AccessLogicalExpression::Not(underlying) => {
                let underlying_predicate =
                    self.solve(request_context, input_context, underlying).await;
                underlying_predicate.map(|p| p.not())
            }
            AccessLogicalExpression::And(left, right) => {
                let left_predicate = self.solve(request_context, input_context, left).await;
                let right_predicate = self.solve(request_context, input_context, right).await;

                match (left_predicate, right_predicate) {
                    (Some(left_predicate), Some(right_predicate)) => {
                        Some(left_predicate.and(right_predicate))
                    }
                    _ => None,
                }
            }
            AccessLogicalExpression::Or(left, right) => {
                let left_predicate = self.solve(request_context, input_context, left).await;
                let right_predicate = self.solve(request_context, input_context, right).await;

                match (left_predicate, right_predicate) {
                    (Some(left_predicate), Some(right_predicate)) => {
                        Some(left_predicate.or(right_predicate))
                    }
                    (Some(left_predicate), None) => Some(left_predicate),
                    (None, Some(right_predicate)) => Some(right_predicate),
                    (None, None) => None,
                }
            }
        }
    }
}

/// A primitive expression that has been reduced to a JSON value or an unresolved context

pub async fn reduce_common_primitive_expression<'a>(
    context_extractor: &(impl ContextExtractor + Send + Sync),
    request_context: &RequestContext<'a>,
    expr: &'a CommonAccessPrimitiveExpression,
) -> Option<Val> {
    match expr {
        CommonAccessPrimitiveExpression::ContextSelection(selection) => context_extractor
            .extract_context_selection(request_context, selection)
            .await
            .unwrap()
            .cloned(),
        CommonAccessPrimitiveExpression::StringLiteral(value) => Some(Val::String(value.clone())),
        CommonAccessPrimitiveExpression::BooleanLiteral(value) => Some(Val::Bool(*value)),
        CommonAccessPrimitiveExpression::NumberLiteral(value) => Some(Val::Number((*value).into())),
    }
}

pub fn eq_values(left_value: &Val, right_value: &Val) -> bool {
    match (left_value, right_value) {
        (Val::Number(left_number), Val::Number(right_number)) => {
            // We have a more general implementation of `PartialEq` for `Val` that accounts for
            // different number types. So, we use that implementaiton here instead of using just `==`
            NumberWrapper(left_number.clone()).partial_cmp(&NumberWrapper(right_number.clone()))
                == Some(std::cmp::Ordering::Equal)
        }
        _ => left_value == right_value,
    }
}

pub fn neq_values(left_value: &Val, right_value: &Val) -> bool {
    !eq_values(left_value, right_value)
}

pub fn in_values(left_value: &Val, right_value: &Val) -> bool {
    match right_value {
        Val::List(values) => values.contains(left_value),
        _ => unreachable!("The right side operand of `in` operator must be an array"), // This never happens see relational_op::in_relation_match
    }
}

pub fn lt_values(left_value: &Val, right_value: &Val) -> bool {
    match (left_value, right_value) {
        (Val::Number(left_number), Val::Number(right_number)) => {
            NumberWrapper(left_number.clone()) < NumberWrapper(right_number.clone())
        }
        _ => unreachable!("The operands of `<` operator must be numbers"),
    }
}

pub fn lte_values(left_value: &Val, right_value: &Val) -> bool {
    match (left_value, right_value) {
        (Val::Number(left_number), Val::Number(right_number)) => {
            NumberWrapper(left_number.clone()) <= NumberWrapper(right_number.clone())
        }
        _ => unreachable!("The operands of `<=` operator must be numbers"),
    }
}

pub fn gt_values(left_value: &Val, right_value: &Val) -> bool {
    !lte_values(left_value, right_value)
}

pub fn gte_values(left_value: &Val, right_value: &Val) -> bool {
    !lt_values(left_value, right_value)
}
