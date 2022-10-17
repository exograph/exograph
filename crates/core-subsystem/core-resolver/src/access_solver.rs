use async_trait::async_trait;
use core_model::access::{
    AccessContextSelection, AccessLogicalExpression, AccessPredicateExpression, AccessRelationalOp,
};
use serde_json::Value;

pub trait AccessPredicate<'a>:
    From<bool> + std::ops::Not<Output = Self> + 'a + Send + Sync
{
    fn and(self, other: Self) -> Self;
    fn or(self, other: Self) -> Self;
}

/// Solve access control logic.
///
/// # Parameters
/// - PrimExpr: Primitive Expression
/// - Res: Result type
#[async_trait]
pub trait AccessSolver<'a, PrimExpr, Res>
where
    PrimExpr: Send + Sync,
    Res: AccessPredicate<'a>,
{
    async fn extract_context(&self, context_name: &str) -> Option<Value>;

    /// Solve access control logic.
    /// The access control logic is expressed as a predicate expression. This method
    /// tries to produce a simplest possible `Predicate` given the request context. It tries
    /// to produce `Predicate::True` or `Predicate::False` when sufficient information is available
    /// to make such determination. This allows (in case of `Predicate::True`) to skip the database
    /// filtering and (in case of `Predicate::False`) to return a "Not authorized" error (instead of an
    /// empty/null result).
    async fn solve(&self, expr: &'a AccessPredicateExpression<PrimExpr>) -> Res {
        match expr {
            AccessPredicateExpression::LogicalOp(op) => self.solve_logical_op(op).await,
            AccessPredicateExpression::RelationalOp(op) => self.solve_relational_op(op).await,
            AccessPredicateExpression::BooleanLiteral(value) => (*value).into(),
        }
    }

    async fn extract_context_selection(
        &self,
        context_selection: &AccessContextSelection,
    ) -> Option<Value> {
        match context_selection {
            AccessContextSelection::Context(context_name) => {
                self.extract_context(context_name).await
            }
            AccessContextSelection::Select(path, key) => self
                .extract_context_selection(path)
                .await
                .and_then(|value| value.get(key).cloned()),
        }
    }

    async fn solve_relational_op(&self, op: &'a AccessRelationalOp<PrimExpr>) -> Res;

    async fn solve_logical_op(&self, op: &'a AccessLogicalExpression<PrimExpr>) -> Res {
        match op {
            AccessLogicalExpression::Not(underlying) => {
                let underlying_predicate = self.solve(underlying).await;
                underlying_predicate.not()
            }
            AccessLogicalExpression::And(left, right) => {
                let left_predicate = self.solve(left).await;
                let right_predicate = self.solve(right).await;

                left_predicate.and(right_predicate)
            }
            AccessLogicalExpression::Or(left, right) => {
                let left_predicate = self.solve(left).await;
                let right_predicate = self.solve(right).await;

                left_predicate.or(right_predicate)
            }
        }
    }
}
