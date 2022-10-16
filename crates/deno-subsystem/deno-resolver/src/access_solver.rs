use std::ops::Not;

use async_recursion::async_recursion;
use core_resolver::request_context::RequestContext;
use deno_model::{
    access::{
        AccessContextSelection, AccessLogicalExpression, AccessPredicateExpression,
        AccessPrimitiveExpression, AccessRelationalOp,
    },
    model::ModelDenoSystem,
};

use serde_json::Value;

use crate::service_access_predicate::ServiceAccessPredicate;

/// Solve access control logic.
/// The access control logic is expressed as a predicate expression. This method
/// tries to produce a simplest possible `Predicate` given the request context. It tries
/// to produce `Predicate::True` or `Predicate::False` when sufficient information is available
/// to make such determination. This allows (in case of `Predicate::True`) to skip the service
/// filtering and (in case of `Predicate::False`) to return a "Not authorized" error (instead of an
/// empty/null result).
pub async fn solve_access<'s, 'a>(
    expr: &'a AccessPredicateExpression,
    request_context: &'a RequestContext<'a>,
    system: &'a ModelDenoSystem,
) -> ServiceAccessPredicate {
    solve_predicate_expression(expr, request_context, system).await
}

#[async_recursion]
async fn solve_predicate_expression<'a>(
    expr: &'a AccessPredicateExpression,
    request_context: &'a RequestContext<'a>,
    system: &'a ModelDenoSystem,
) -> ServiceAccessPredicate {
    match expr {
        AccessPredicateExpression::LogicalOp(op) => {
            solve_logical_op(op, request_context, system).await
        }
        AccessPredicateExpression::RelationalOp(op) => {
            solve_relational_op(op, request_context, system).await
        }
        AccessPredicateExpression::BooleanLiteral(value) => (*value).into(),
    }
}

#[async_recursion]
async fn solve_context_selection<'a>(
    context_selection: &AccessContextSelection,
    value: &'a RequestContext<'a>,
    system: &'a ModelDenoSystem,
) -> Option<Value> {
    match context_selection {
        AccessContextSelection::Context(context_name) => {
            let context_type = system.contexts.get_by_key(context_name).unwrap();
            value.extract_context(context_type).await.ok()
        }
        AccessContextSelection::Select(path, key) => solve_context_selection(path, value, system)
            .await
            .and_then(|value| value.get(key).cloned()),
    }
}

async fn solve_relational_op<'a>(
    op: &'a AccessRelationalOp,
    request_context: &'a RequestContext<'a>,
    system: &'a ModelDenoSystem,
) -> ServiceAccessPredicate {
    #[derive(Debug)]
    enum SolvedPrimitiveExpression<'a> {
        Value(Value),
        UnresolvedContext(&'a AccessContextSelection), // For example, AuthContext.role for an anonymous user
    }

    async fn reduce_primitive_expression<'a>(
        expr: &'a AccessPrimitiveExpression,
        request_context: &'a RequestContext<'a>,
        system: &'a ModelDenoSystem,
    ) -> SolvedPrimitiveExpression<'a> {
        match expr {
            AccessPrimitiveExpression::ContextSelection(selection) => {
                solve_context_selection(selection, request_context, system)
                    .await
                    .map(SolvedPrimitiveExpression::Value)
                    .unwrap_or(SolvedPrimitiveExpression::UnresolvedContext(selection))
            }
            AccessPrimitiveExpression::StringLiteral(value) => {
                SolvedPrimitiveExpression::Value(Value::String(value.clone()))
            }
            AccessPrimitiveExpression::BooleanLiteral(value) => {
                SolvedPrimitiveExpression::Value(Value::Bool(*value))
            }
            AccessPrimitiveExpression::NumberLiteral(value) => {
                SolvedPrimitiveExpression::Value(Value::Number((*value).into()))
            }
        }
    }

    let (left, right) = op.sides();
    let left = reduce_primitive_expression(left, request_context, system).await;
    let right = reduce_primitive_expression(right, request_context, system).await;

    type ValuePredicateFn<'a> = fn(Value, Value) -> ServiceAccessPredicate;

    let helper = |unresolved_context_predicate: ServiceAccessPredicate,
                  value_predicate: ValuePredicateFn<'a>|
     -> ServiceAccessPredicate {
        match (left, right) {
            (SolvedPrimitiveExpression::UnresolvedContext(_), _)
            | (_, SolvedPrimitiveExpression::UnresolvedContext(_)) => unresolved_context_predicate,
            (
                SolvedPrimitiveExpression::Value(left_value),
                SolvedPrimitiveExpression::Value(right_value),
            ) => value_predicate(left_value, right_value),
        }
    };

    match op {
        AccessRelationalOp::Eq(..) => helper(ServiceAccessPredicate::False, |val1, val2| {
            (val1 == val2).into()
        }),
        AccessRelationalOp::Neq(_, _) => helper(
            ServiceAccessPredicate::True, // If a context is undefined, declare the expression as a match. For example, `AuthContext.role != "ADMIN"` for anonymous user evaluates to true
            |val1, val2| (val1 != val2).into(),
        ),
        AccessRelationalOp::In(..) => helper(
            ServiceAccessPredicate::False,
            |left_value, right_value| match right_value {
                Value::Array(values) => values.contains(&left_value).into(),
                _ => unreachable!("The right side operand of `in` operator must be an array"), // This never happens see relational_op::in_relation_match
            },
        ),
        _ => unreachable!("Unsupported relational operator"),
    }
}

async fn solve_logical_op<'a>(
    op: &'a AccessLogicalExpression,
    request_context: &'a RequestContext<'a>,
    system: &'a ModelDenoSystem,
) -> ServiceAccessPredicate {
    match op {
        AccessLogicalExpression::Not(underlying) => {
            let underlying_predicate =
                solve_predicate_expression(underlying, request_context, system).await;
            underlying_predicate.not()
        }
        AccessLogicalExpression::And(left, right) => {
            let left_predicate = solve_predicate_expression(left, request_context, system).await;
            let right_predicate = solve_predicate_expression(right, request_context, system).await;

            (left_predicate.into() && right_predicate.into()).into()
        }
        AccessLogicalExpression::Or(left, right) => {
            let left_predicate = solve_predicate_expression(left, request_context, system).await;
            let right_predicate = solve_predicate_expression(right, request_context, system).await;

            (left_predicate.into() || right_predicate.into()).into()
        }
    }
}
