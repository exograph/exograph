use async_trait::async_trait;
use core_model::access::{
    AccessContextSelection, AccessLogicalExpression, AccessPredicateExpression, AccessRelationalOp,
};
use serde_json::{Map, Value};

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
    async fn solve(&self, expr: &'a AccessPredicateExpression<PrimExpr>) -> Res {
        match expr {
            AccessPredicateExpression::LogicalOp(op) => self.solve_logical_op(op).await,
            AccessPredicateExpression::RelationalOp(op) => self.solve_relational_op(op).await,
            AccessPredicateExpression::BooleanLiteral(value) => (*value).into(),
        }
    }

    /// Extract the context object.
    ///
    /// If the context type is defined as:
    ///
    /// ```clay
    /// context AuthContext {
    ///   id: Int
    ///   name: String
    ///   role: String
    /// }
    /// ```
    ///
    /// Then calling this with `context_name` set to `"AuthContext"` will return an object
    /// such as:
    ///
    /// ```json
    /// {
    ///   id: 1,
    ///   name: "John",
    ///   role: "admin",
    /// }
    /// ```
    async fn extract_context(&self, context_name: &str) -> Option<Map<String, Value>>;

    /// Extract the context object selection.
    ///
    /// This method is similar to `extract_context` but it allows to select a specific field from
    /// the context object. For example, consider the context type and the context object in the
    /// documentation of [`extract_context`](AccessSolver::extract_context). Calling this method
    /// with `context_selection` set to
    /// `AccessContextSelection::Select(AccessContextSelection("AuthContext"), "role")` will return
    /// the value `"admin"`.
    async fn extract_context_selection(
        &self,
        context_selection: &AccessContextSelection,
    ) -> Option<Value> {
        fn extract_path<'a>(value: &'a Value, path: &[String]) -> Option<&'a Value> {
            match path.split_first() {
                Some((key, tail)) => value.get(key).and_then(|value| extract_path(value, tail)),
                None => Some(value),
            }
        }

        let context = self
            .extract_context(&context_selection.context_name)
            .await?;
        context
            .get(&context_selection.path.0)
            .and_then(|head_selection| extract_path(head_selection, &context_selection.path.1))
            .cloned()
    }

    /// Solve relational operation such as `=`, `!=`, `<`, `>`, `<=`, `>=`.
    ///
    /// Since relating two primitive expressions depend on the subsystem, this method is abstract.
    /// For example, a database subsystem produce a relational expression comparing two columns
    /// such as `column_a < column_b`.
    async fn solve_relational_op(&self, op: &'a AccessRelationalOp<PrimExpr>) -> Res;

    /// Solve logical operations such as `not`, `and`, `or`.
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
