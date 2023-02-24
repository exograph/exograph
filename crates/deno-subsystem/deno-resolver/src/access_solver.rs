use async_trait::async_trait;

use core_plugin_interface::{
    core_model::{
        access::{AccessContextSelection, AccessRelationalOp},
        context_type::ContextType,
        mapped_arena::MappedArena,
    },
    core_resolver::{
        access_solver::{AccessPredicate, AccessSolver},
        request_context::RequestContext,
    },
};

use deno_model::{access::ServiceAccessPrimitiveExpression, subsystem::DenoSubsystem};

use serde_json::Value;

use crate::service_access_predicate::ServiceAccessPredicate;

// Only to get around the orphan rule while implementing AccessSolver
pub struct ServiceAccessPredicateWrapper(pub ServiceAccessPredicate);

impl std::ops::Not for ServiceAccessPredicateWrapper {
    type Output = Self;

    fn not(self) -> Self::Output {
        ServiceAccessPredicateWrapper(self.0.not())
    }
}

impl From<bool> for ServiceAccessPredicateWrapper {
    fn from(value: bool) -> Self {
        ServiceAccessPredicateWrapper(ServiceAccessPredicate::from(value))
    }
}

impl<'a> AccessPredicate<'a> for ServiceAccessPredicateWrapper {
    fn and(self, other: Self) -> Self {
        ServiceAccessPredicateWrapper((self.0.into() && other.0.into()).into())
    }

    fn or(self, other: Self) -> Self {
        ServiceAccessPredicateWrapper((self.0.into() || other.0.into()).into())
    }
}

#[async_trait]
impl<'a> AccessSolver<'a, ServiceAccessPrimitiveExpression, ServiceAccessPredicateWrapper>
    for DenoSubsystem
{
    fn contexts(&self) -> &MappedArena<ContextType> {
        &self.contexts
    }

    async fn solve_relational_op(
        &'a self,
        request_context: &'a RequestContext<'a>,
        op: &'a AccessRelationalOp<ServiceAccessPrimitiveExpression>,
    ) -> ServiceAccessPredicateWrapper {
        #[derive(Debug)]
        enum SolvedPrimitiveExpression<'a> {
            Value(Value),
            UnresolvedContext(&'a AccessContextSelection), // For example, AuthContext.role for an anonymous user
        }

        async fn reduce_primitive_expression<'a>(
            solver: &DenoSubsystem,
            request_context: &'a RequestContext<'a>,
            expr: &'a ServiceAccessPrimitiveExpression,
        ) -> SolvedPrimitiveExpression<'a> {
            match expr {
                ServiceAccessPrimitiveExpression::ContextSelection(selection) => solver
                    .extract_context_selection(request_context, selection)
                    .await
                    .map(SolvedPrimitiveExpression::Value)
                    .unwrap_or(SolvedPrimitiveExpression::UnresolvedContext(selection)),
                ServiceAccessPrimitiveExpression::StringLiteral(value) => {
                    SolvedPrimitiveExpression::Value(Value::String(value.clone()))
                }
                ServiceAccessPrimitiveExpression::BooleanLiteral(value) => {
                    SolvedPrimitiveExpression::Value(Value::Bool(*value))
                }
                ServiceAccessPrimitiveExpression::NumberLiteral(value) => {
                    SolvedPrimitiveExpression::Value(Value::Number((*value).into()))
                }
            }
        }

        let (left, right) = op.sides();
        let left = reduce_primitive_expression(self, request_context, left).await;
        let right = reduce_primitive_expression(self, request_context, right).await;

        type ValuePredicateFn<'a> = fn(Value, Value) -> ServiceAccessPredicate;

        let helper = |unresolved_context_predicate: ServiceAccessPredicate,
                      value_predicate: ValuePredicateFn<'a>|
         -> ServiceAccessPredicate {
            match (left, right) {
                (SolvedPrimitiveExpression::UnresolvedContext(_), _)
                | (_, SolvedPrimitiveExpression::UnresolvedContext(_)) => {
                    unresolved_context_predicate
                }
                (
                    SolvedPrimitiveExpression::Value(left_value),
                    SolvedPrimitiveExpression::Value(right_value),
                ) => value_predicate(left_value, right_value),
            }
        };

        ServiceAccessPredicateWrapper(match op {
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
        })
    }
}
