use async_trait::async_trait;
use core_model::access::AccessContextSelection;
use core_resolver::request_context::RequestContext;
use maybe_owned::MaybeOwned;
use postgres_model::{
    access::{
        AccessLogicalExpression, AccessPredicateExpression, AccessPrimitiveExpression,
        AccessRelationalOp,
    },
    column_path::ColumnIdPath,
    model::ModelPostgresSystem,
};

use crate::column_path_util;
use payas_sql::{AbstractPredicate, ColumnPath};

use serde_json::Value;

pub trait AccessPredicate<'a>:
    From<bool> + std::ops::Not<Output = Self> + 'a + Send + Sync
{
    fn and(self, other: Self) -> Self;
    fn or(self, other: Self) -> Self;
}

impl<'a> AccessPredicate<'a> for AbstractPredicate<'a> {
    fn and(self, other: Self) -> Self {
        AbstractPredicate::and(self, other)
    }

    fn or(self, other: Self) -> Self {
        AbstractPredicate::or(self, other)
    }
}

/// Solve access control logic.
///
/// # Parameters
/// - PrimExpr: Primitive Expression
/// - Res: Result type
#[async_trait]
pub trait AccessSolver<'a, _PrimExpr, Res>
where
    Res: AccessPredicate<'a>,
{
    async fn extract_context(&self, context_name: &str) -> Option<Value>;
    fn to_column_path(&self, path: &ColumnIdPath) -> ColumnPath<'a>;

    /// Solve access control logic.
    /// The access control logic is expressed as a predicate expression. This method
    /// tries to produce a simplest possible `Predicate` given the request context. It tries
    /// to produce `Predicate::True` or `Predicate::False` when sufficient information is available
    /// to make such determination. This allows (in case of `Predicate::True`) to skip the database
    /// filtering and (in case of `Predicate::False`) to return a "Not authorized" error (instead of an
    /// empty/null result).
    async fn solve(&self, expr: &'a AccessPredicateExpression) -> Res {
        match expr {
            AccessPredicateExpression::LogicalOp(op) => self.solve_logical_op(op).await,
            AccessPredicateExpression::RelationalOp(op) => self.solve_relational_op(op).await,
            AccessPredicateExpression::BooleanLiteral(value) => (*value).into(),
        }
    }

    async fn solve_context_selection(
        &self,
        context_selection: &AccessContextSelection,
    ) -> Option<Value> {
        match context_selection {
            AccessContextSelection::Context(context_name) => {
                self.extract_context(context_name).await
            }
            AccessContextSelection::Select(path, key) => self
                .solve_context_selection(path)
                .await
                .and_then(|value| value.get(key).cloned()),
        }
    }

    async fn solve_relational_op(&self, op: &'a AccessRelationalOp) -> Res;

    async fn solve_logical_op(&self, op: &'a AccessLogicalExpression) -> Res {
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

    async fn reduce_primitive_expression(
        &self,
        expr: &'a AccessPrimitiveExpression,
    ) -> SolvedPrimitiveExpression<'a> {
        match expr {
            AccessPrimitiveExpression::ContextSelection(selection) => self
                .solve_context_selection(selection)
                .await
                .map(SolvedPrimitiveExpression::Value)
                .unwrap_or(SolvedPrimitiveExpression::UnresolvedContext(selection)),
            AccessPrimitiveExpression::Column(column_path) => {
                SolvedPrimitiveExpression::Column(column_path.clone())
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
}

#[derive(Debug)]
pub enum SolvedPrimitiveExpression<'a> {
    Value(Value),
    Column(ColumnIdPath),
    UnresolvedContext(&'a AccessContextSelection), // For example, AuthContext.role for an anonymous user
}

pub struct PostgresAccessSolver<'a> {
    request_context: &'a RequestContext<'a>,
    system: &'a ModelPostgresSystem,
}

impl PostgresAccessSolver<'_> {
    pub fn new<'a>(
        request_context: &'a RequestContext<'a>,
        system: &'a ModelPostgresSystem,
    ) -> PostgresAccessSolver<'a> {
        PostgresAccessSolver {
            request_context,
            system,
        }
    }
}

#[async_trait]
impl<'a> AccessSolver<'a, AccessPrimitiveExpression, AbstractPredicate<'a>>
    for PostgresAccessSolver<'a>
{
    async fn extract_context(&self, context_name: &str) -> Option<Value> {
        let context_type = self.system.contexts.get_by_key(context_name).unwrap();
        self.request_context
            .extract_context(context_type)
            .await
            .ok()
    }

    fn to_column_path(&self, path: &ColumnIdPath) -> ColumnPath<'a> {
        to_column_path(path, self.system)
    }

    async fn solve_relational_op(&self, op: &'a AccessRelationalOp) -> AbstractPredicate<'a> {
        let (left, right) = op.sides();
        let left = self.reduce_primitive_expression(left).await;
        let right = self.reduce_primitive_expression(right).await;

        type ColumnPredicateFn<'a> = fn(
            MaybeOwned<'a, ColumnPath<'a>>,
            MaybeOwned<'a, ColumnPath<'a>>,
        ) -> AbstractPredicate<'a>;
        type ValuePredicateFn<'a> = fn(Value, Value) -> AbstractPredicate<'a>;

        let helper = |unresolved_context_predicate: AbstractPredicate<'a>,
                      column_predicate: ColumnPredicateFn<'a>,
                      value_predicate: ValuePredicateFn<'a>|
         -> AbstractPredicate<'a> {
            match (left, right) {
                (SolvedPrimitiveExpression::UnresolvedContext(_), _)
                | (_, SolvedPrimitiveExpression::UnresolvedContext(_)) => {
                    unresolved_context_predicate
                }
                (
                    SolvedPrimitiveExpression::Column(left_col),
                    SolvedPrimitiveExpression::Column(right_col),
                ) => column_predicate(
                    self.to_column_path(&left_col).into(),
                    self.to_column_path(&right_col).into(),
                ),

                (
                    SolvedPrimitiveExpression::Value(left_value),
                    SolvedPrimitiveExpression::Value(right_value),
                ) => value_predicate(left_value, right_value),
                (
                    SolvedPrimitiveExpression::Value(value),
                    SolvedPrimitiveExpression::Column(column),
                )
                | (
                    SolvedPrimitiveExpression::Column(column),
                    SolvedPrimitiveExpression::Value(value),
                ) => column_predicate(self.to_column_path(&column).into(), literal_column(value)),
            }
        };

        match op {
            AccessRelationalOp::Eq(..) => helper(
                AbstractPredicate::False,
                AbstractPredicate::eq,
                |val1, val2| (val1 == val2).into(),
            ),
            AccessRelationalOp::Neq(_, _) => helper(
                AbstractPredicate::True, // If a context is undefined, declare the expression as a match. For example, `AuthContext.role != "ADMIN"` for anonymous user evaluates to true
                AbstractPredicate::neq,
                |val1, val2| (val1 != val2).into(),
            ),
            // For the next four, we could better optimize cases where values are known, but for now, we generate a predicate and let database handle it
            AccessRelationalOp::Lt(_, _) => helper(
                AbstractPredicate::False,
                AbstractPredicate::Lt,
                |val1, val2| AbstractPredicate::Lt(literal_column(val1), literal_column(val2)),
            ),
            AccessRelationalOp::Lte(_, _) => helper(
                AbstractPredicate::False,
                AbstractPredicate::Lte,
                |val1, val2| AbstractPredicate::Lte(literal_column(val1), literal_column(val2)),
            ),
            AccessRelationalOp::Gt(_, _) => helper(
                AbstractPredicate::False,
                AbstractPredicate::Gt,
                |val1, val2| AbstractPredicate::Gt(literal_column(val1), literal_column(val2)),
            ),
            AccessRelationalOp::Gte(_, _) => helper(
                AbstractPredicate::False,
                AbstractPredicate::Gte,
                |val1, val2| AbstractPredicate::Gte(literal_column(val1), literal_column(val2)),
            ),
            AccessRelationalOp::In(..) => helper(
                AbstractPredicate::False,
                AbstractPredicate::In,
                |left_value, right_value| match right_value {
                    Value::Array(values) => values.contains(&left_value).into(),
                    _ => unreachable!("The right side operand of `in` operator must be an array"), // This never happens see relational_op::in_relation_match
                },
            ),
        }
    }
}

fn to_column_path<'a>(column_id: &ColumnIdPath, system: &'a ModelPostgresSystem) -> ColumnPath<'a> {
    column_path_util::to_column_path(&Some(column_id.clone()), &None, system)
}

fn literal_column(value: Value) -> MaybeOwned<'static, ColumnPath<'static>> {
    match value {
        Value::Null => ColumnPath::Null,
        Value::Bool(v) => ColumnPath::Literal(MaybeOwned::Owned(Box::new(v))),
        Value::Number(v) => {
            ColumnPath::Literal(MaybeOwned::Owned(Box::new(v.as_i64().unwrap() as i32)))
        } // TODO: Deal with the exact number type
        Value::String(v) => ColumnPath::Literal(MaybeOwned::Owned(Box::new(v))),
        Value::Array(values) => ColumnPath::Literal(MaybeOwned::Owned(Box::new(values))),
        Value::Object(_) => todo!(),
    }
    .into()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use core_plugin::interception::InterceptionMap;
    use core_resolver::introspection::definition::schema::Schema;
    use core_resolver::request_context::Request;
    use core_resolver::system_resolver::SystemResolver;
    use postgres_model::{column_id::ColumnId, column_path::ColumnIdPathLink};
    use serde_json::json;

    use super::*;

    struct TestSystem {
        system: ModelPostgresSystem,
        published_column_path: ColumnIdPath,
        owner_id_column_path: ColumnIdPath,
        dept1_id_column_path: ColumnIdPath,
        dept2_id_column_path: ColumnIdPath,
        test_system_resolver: SystemResolver,
    }

    struct TestRequest {}

    impl Request for TestRequest {
        fn get_headers(&self, _key: &str) -> Vec<String> {
            vec![]
        }

        fn get_ip(&self) -> Option<std::net::IpAddr> {
            None
        }
    }

    const REQUEST: TestRequest = TestRequest {};

    impl TestSystem {
        fn published_column(&self) -> MaybeOwned<ColumnPath> {
            super::to_column_path(&self.published_column_path, &self.system).into()
        }

        fn owner_id_column(&self) -> MaybeOwned<ColumnPath> {
            super::to_column_path(&self.owner_id_column_path, &self.system).into()
        }

        fn dept1_id_column(&self) -> MaybeOwned<ColumnPath> {
            super::to_column_path(&self.dept1_id_column_path, &self.system).into()
        }

        fn dept2_id_column(&self) -> MaybeOwned<ColumnPath> {
            super::to_column_path(&self.dept2_id_column_path, &self.system).into()
        }
    }

    fn test_system() -> TestSystem {
        let postgres_subsystem = crate::test_utils::create_postgres_system_from_str(
            r#"
                context AccessContext {
                    role: String @test("role")
                    token1: String @test("token1")
                    token2: String @test("token2")
                    is_admin: Boolean @test("is_admin")
                    user_id: String @test("user_id")
                    v1: Boolean @test("v1")
                    v2: Boolean @test("v2")
                    v1_clone: Boolean @test("v1_clone")
                    v2_clone: Boolean @test("v2_clone")
                }

                model Article {
                    id: Int = autoincrement() @pk
                    published: Boolean
                    owner_id: Int @bits(64)
                    dept1_id: Int @bits(64)
                    dept2_id: Int @bits(64)
                }
            "#,
            "test.clay".to_string(),
        )
        .unwrap();
        let (table_id, table) = postgres_subsystem
            .tables
            .iter()
            .find(|table| table.1.name == "articles")
            .unwrap();

        let get_column_id = |column_name: &str| {
            let column_index = table.column_index(column_name).unwrap();

            ColumnId::new(table_id, column_index)
        };

        let published_column_id = get_column_id("published");
        let owner_id_column_id = get_column_id("owner_id");
        let dept1_id_column_id = get_column_id("dept1_id");
        let dept2_id_column_id = get_column_id("dept2_id");

        let published_column_path = ColumnIdPath {
            path: vec![ColumnIdPathLink {
                self_column_id: published_column_id,
                linked_column_id: None,
            }],
        };

        let owner_id_column_path = ColumnIdPath {
            path: vec![ColumnIdPathLink {
                self_column_id: owner_id_column_id,
                linked_column_id: None,
            }],
        };

        let dept1_id_column_path = ColumnIdPath {
            path: vec![ColumnIdPathLink {
                self_column_id: dept1_id_column_id,
                linked_column_id: None,
            }],
        };

        let dept2_id_column_path = ColumnIdPath {
            path: vec![ColumnIdPathLink {
                self_column_id: dept2_id_column_id,
                linked_column_id: None,
            }],
        };

        // Create an empty SystemResolver. Since in tests we never invoke the resolver (since we don't have @query context),
        // we don't need to populate it.
        let test_system_resolver = SystemResolver::new(
            vec![],
            InterceptionMap {
                map: HashMap::new(),
            },
            InterceptionMap {
                map: HashMap::new(),
            },
            Schema::new(vec![], vec![], vec![]),
        );

        TestSystem {
            system: postgres_subsystem,
            published_column_path,
            owner_id_column_path,
            dept1_id_column_path,
            dept2_id_column_path,
            test_system_resolver,
        }
    }

    fn context_selection(head: &str, tail: &[&str]) -> AccessContextSelection {
        match tail {
            [] => AccessContextSelection::Context(head.to_string()),
            [init @ .., last] => AccessContextSelection::Select(
                Box::new(context_selection(head, init)),
                last.to_string(),
            ),
        }
    }

    fn context_selection_expr(head: &str, tail: &[&str]) -> Box<AccessPrimitiveExpression> {
        Box::new(AccessPrimitiveExpression::ContextSelection(
            context_selection(head, tail),
        ))
    }

    // AuthContext.is_admin => AuthContext.is_admin == true
    fn boolean_context_selection(
        context_selection: AccessContextSelection,
    ) -> AccessPredicateExpression {
        AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
            Box::new(AccessPrimitiveExpression::ContextSelection(
                context_selection,
            )),
            Box::new(AccessPrimitiveExpression::BooleanLiteral(true)),
        ))
    }

    // self.published => self.published == true
    fn boolean_column_selection(column_path: ColumnIdPath) -> AccessPredicateExpression {
        AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
            Box::new(AccessPrimitiveExpression::Column(column_path)),
            Box::new(AccessPrimitiveExpression::BooleanLiteral(true)),
        ))
    }

    async fn solve_access<'a>(
        expr: &'a AccessPredicateExpression,
        request_context: &'a RequestContext<'a>,
        subsystem_model: &'a ModelPostgresSystem,
    ) -> AbstractPredicate<'a> {
        let access_solver = PostgresAccessSolver::new(request_context, subsystem_model);
        access_solver.solve(expr).await
    }

    async fn test_relational_op<'a>(
        test_system: &'a TestSystem,
        op: fn(
            Box<AccessPrimitiveExpression>,
            Box<AccessPrimitiveExpression>,
        ) -> AccessRelationalOp,
        context_match_predicate: fn(
            MaybeOwned<'a, ColumnPath<'a>>,
            MaybeOwned<'a, ColumnPath<'a>>,
        ) -> AbstractPredicate<'a>,
        context_mismatch_predicate: fn(
            MaybeOwned<'a, ColumnPath<'a>>,
            MaybeOwned<'a, ColumnPath<'a>>,
        ) -> AbstractPredicate<'a>,
        context_missing_predicate: AbstractPredicate<'a>,
        context_value_predicate: fn(
            MaybeOwned<'a, ColumnPath<'a>>,
            MaybeOwned<'a, ColumnPath<'a>>,
        ) -> AbstractPredicate<'a>,
        column_column_predicate: fn(
            MaybeOwned<'a, ColumnPath<'a>>,
            MaybeOwned<'a, ColumnPath<'a>>,
        ) -> AbstractPredicate<'a>,
    ) {
        let TestSystem {
            system,
            owner_id_column_path,
            dept1_id_column_path,
            dept2_id_column_path,
            test_system_resolver,
            ..
        } = &test_system;

        // Case 1: Both values from AuthContext
        {
            let test_ae = AccessPredicateExpression::RelationalOp(op(
                context_selection_expr("AccessContext", &["token1"]),
                context_selection_expr("AccessContext", &["token2"]),
            ));

            let request_context = test_request_context(
                json!({"token1": "token_value", "token2": "token_value"}),
                test_system_resolver,
            );
            let solved_predicate = solve_access(&test_ae, &request_context, system).await;
            assert_eq!(
                solved_predicate,
                context_match_predicate(
                    ColumnPath::Literal(MaybeOwned::Owned(Box::new("token_value".to_string())))
                        .into(),
                    ColumnPath::Literal(MaybeOwned::Owned(Box::new("token_value".to_string())))
                        .into(),
                )
            );

            // The mismatch case doesn't make sense for lt/lte/gt/gte, but since we don't optimize
            // (to reduce obvious matches such as 5 < 6 => Predicate::True/False) in those cases,
            // the unoptimized predicate created works for both match and mismatch cases.

            let request_context = test_request_context(
                json!({"token1": "token_value1", "token2": "token_value2"}),
                test_system_resolver,
            );
            let solved_predicate = solve_access(&test_ae, &request_context, system).await;
            assert_eq!(
                solved_predicate,
                context_mismatch_predicate(
                    ColumnPath::Literal(MaybeOwned::Owned(Box::new("token_value1".to_string())))
                        .into(),
                    ColumnPath::Literal(MaybeOwned::Owned(Box::new("token_value2".to_string())))
                        .into(),
                )
            );
        }

        // One value from AuthContext and other from a column
        {
            let test_context_column = |test_ae: AccessPredicateExpression| async {
                let test_ae = test_ae;
                let context = test_request_context(json!({"user_id": "u1"}), test_system_resolver);
                let solved_predicate = solve_access(&test_ae, &context, system).await;
                assert_eq!(
                    solved_predicate,
                    context_value_predicate(
                        test_system.owner_id_column(),
                        ColumnPath::Literal(MaybeOwned::Owned(Box::new("u1".to_string()))).into(),
                    )
                );

                // No user_id, so we can definitely declare it Predicate::False
                let context = test_request_context(json!({}), test_system_resolver);
                let solved_predicate = solve_access(&test_ae, &context, system).await;
                assert_eq!(&solved_predicate, &context_missing_predicate);
            };

            // Once test with `context op column` and then `column op context`
            test_context_column(AccessPredicateExpression::RelationalOp(op(
                context_selection_expr("AccessContext", &["user_id"]),
                Box::new(AccessPrimitiveExpression::Column(
                    owner_id_column_path.clone(),
                )),
            )))
            .await;

            test_context_column(AccessPredicateExpression::RelationalOp(op(
                Box::new(AccessPrimitiveExpression::Column(
                    owner_id_column_path.clone(),
                )),
                context_selection_expr("AccessContext", &["user_id"]),
            )))
            .await;
        }

        // Both values from columns
        {
            let test_ae = AccessPredicateExpression::RelationalOp(op(
                Box::new(AccessPrimitiveExpression::Column(
                    dept1_id_column_path.clone(),
                )),
                Box::new(AccessPrimitiveExpression::Column(
                    dept2_id_column_path.clone(),
                )),
            ));

            // context is irrelevant
            let context = test_request_context(Value::Null, test_system_resolver);
            let solved_predicate = solve_access(&test_ae, &context, system).await;
            assert_eq!(
                solved_predicate,
                column_column_predicate(
                    test_system.dept1_id_column(),
                    test_system.dept2_id_column(),
                )
            );
        }
    }

    #[tokio::test]
    async fn basic_eq() {
        test_relational_op(
            &test_system(),
            AccessRelationalOp::Eq,
            |_, _| AbstractPredicate::True,
            |_, _| AbstractPredicate::False,
            AbstractPredicate::False,
            AbstractPredicate::Eq,
            AbstractPredicate::Eq,
        )
        .await;
    }

    #[tokio::test]
    async fn basic_neq() {
        test_relational_op(
            &test_system(),
            AccessRelationalOp::Neq,
            |_, _| AbstractPredicate::False,
            |_, _| AbstractPredicate::True,
            AbstractPredicate::True,
            AbstractPredicate::Neq,
            AbstractPredicate::Neq,
        )
        .await;
    }

    #[tokio::test]
    async fn basic_lt() {
        test_relational_op(
            &test_system(),
            AccessRelationalOp::Lt,
            AbstractPredicate::Lt,
            AbstractPredicate::Lt,
            AbstractPredicate::False,
            AbstractPredicate::Lt,
            AbstractPredicate::Lt,
        )
        .await;
    }

    #[tokio::test]
    async fn basic_lte() {
        test_relational_op(
            &test_system(),
            AccessRelationalOp::Lte,
            AbstractPredicate::Lte,
            AbstractPredicate::Lte,
            AbstractPredicate::False,
            AbstractPredicate::Lte,
            AbstractPredicate::Lte,
        )
        .await;
    }

    #[tokio::test]
    async fn basic_gt() {
        test_relational_op(
            &test_system(),
            AccessRelationalOp::Gt,
            AbstractPredicate::Gt,
            AbstractPredicate::Gt,
            AbstractPredicate::False,
            AbstractPredicate::Gt,
            AbstractPredicate::Gt,
        )
        .await;
    }

    #[tokio::test]
    async fn basic_gte() {
        test_relational_op(
            &test_system(),
            AccessRelationalOp::Gte,
            AbstractPredicate::Gte,
            AbstractPredicate::Gte,
            AbstractPredicate::False,
            AbstractPredicate::Gte,
            AbstractPredicate::Gte,
        )
        .await;
    }

    #[allow(clippy::too_many_arguments)]
    async fn test_logical_op<'a>(
        test_system: &'a TestSystem,
        op: fn(
            Box<AccessPredicateExpression>,
            Box<AccessPredicateExpression>,
        ) -> AccessLogicalExpression,
        both_value_true: AbstractPredicate<'a>,
        both_value_false: AbstractPredicate<'a>,
        one_value_true: AbstractPredicate<'a>,
        one_literal_true_other_column: fn(AbstractPredicate<'a>) -> AbstractPredicate<'a>,
        one_literal_false_other_column: fn(AbstractPredicate<'a>) -> AbstractPredicate<'a>,
        both_columns: fn(
            Box<AbstractPredicate<'a>>,
            Box<AbstractPredicate<'a>>,
        ) -> AbstractPredicate<'a>,
    ) {
        let TestSystem {
            system,
            dept1_id_column_path,
            dept2_id_column_path,
            test_system_resolver,
            ..
        } = &test_system;

        {
            // Two literals
            // context is irrelevant
            let context = test_request_context(Value::Null, test_system_resolver);

            let scenarios = [
                (true, true, &both_value_true),
                (true, false, &one_value_true),
                (false, true, &one_value_true),
                (false, false, &both_value_false),
            ];

            for (l1, l2, expected) in scenarios.iter() {
                let test_ae = AccessPredicateExpression::LogicalOp(op(
                    Box::new(AccessPredicateExpression::BooleanLiteral(*l1)),
                    Box::new(AccessPredicateExpression::BooleanLiteral(*l2)),
                ));

                let solved_predicate = solve_access(&test_ae, &context, system).await;
                assert_eq!(&&solved_predicate, expected);
            }
        }
        {
            // Two context values
            let context = test_request_context(
                json!({"v1": true, "v1_clone": true, "v2": false, "v2_clone": false}),
                test_system_resolver,
            );

            let scenarios = [
                ("v1", "v1_clone", &both_value_true),
                ("v1", "v2", &one_value_true),
                ("v2", "v1", &one_value_true),
                ("v2", "v2_clone", &both_value_false),
            ];

            for (c1, c2, expected) in scenarios.iter() {
                let test_ae = AccessPredicateExpression::LogicalOp(op(
                    Box::new(boolean_context_selection(context_selection(
                        "AccessContext",
                        &[c1],
                    ))),
                    Box::new(boolean_context_selection(context_selection(
                        "AccessContext",
                        &[c2],
                    ))),
                ));

                let solved_predicate = solve_access(&test_ae, &context, system).await;
                assert_eq!(&&solved_predicate, expected);
            }
        }
        {
            // One literal and other a column
            let scenarios = [
                (true, &one_literal_true_other_column),
                (false, &one_literal_false_other_column),
            ];
            let context = test_request_context(Value::Null, test_system_resolver); // context is irrelevant

            for (l, predicate_fn) in scenarios.iter() {
                let test_ae = AccessPredicateExpression::LogicalOp(op(
                    Box::new(AccessPredicateExpression::BooleanLiteral(*l)),
                    Box::new(boolean_column_selection(dept1_id_column_path.clone())),
                ));

                let solved_predicate = solve_access(&test_ae, &context, system).await;
                assert_eq!(
                    solved_predicate,
                    predicate_fn(AbstractPredicate::Eq(
                        test_system.dept1_id_column(),
                        ColumnPath::Literal(MaybeOwned::Owned(Box::new(true))).into()
                    ))
                );

                // The swapped version
                let test_ae = AccessPredicateExpression::LogicalOp(op(
                    Box::new(boolean_column_selection(dept1_id_column_path.clone())),
                    Box::new(AccessPredicateExpression::BooleanLiteral(*l)),
                ));

                let solved_predicate = solve_access(&test_ae, &context, system).await;
                assert_eq!(
                    solved_predicate,
                    predicate_fn(AbstractPredicate::Eq(
                        test_system.dept1_id_column(),
                        ColumnPath::Literal(MaybeOwned::Owned(Box::new(true))).into()
                    ))
                );
            }
        }

        {
            // Two columns
            let test_ae = AccessPredicateExpression::LogicalOp(op(
                Box::new(boolean_column_selection(dept1_id_column_path.clone())),
                Box::new(boolean_column_selection(dept2_id_column_path.clone())),
            ));

            let context = test_request_context(Value::Null, test_system_resolver); // context is irrelevant
            let solved_predicate = solve_access(&test_ae, &context, system).await;
            assert_eq!(
                solved_predicate,
                both_columns(
                    Box::new(AbstractPredicate::Eq(
                        test_system.dept1_id_column(),
                        ColumnPath::Literal(MaybeOwned::Owned(Box::new(true))).into()
                    )),
                    Box::new(AbstractPredicate::Eq(
                        test_system.dept2_id_column(),
                        ColumnPath::Literal(MaybeOwned::Owned(Box::new(true))).into()
                    ))
                )
            );
        }
    }

    #[tokio::test]
    async fn basic_and() {
        test_logical_op(
            &test_system(),
            AccessLogicalExpression::And,
            AbstractPredicate::True,
            AbstractPredicate::False,
            AbstractPredicate::False,
            |p| p,
            |_| AbstractPredicate::False,
            AbstractPredicate::And,
        )
        .await;
    }

    #[tokio::test]
    async fn basic_or() {
        test_logical_op(
            &test_system(),
            AccessLogicalExpression::Or,
            AbstractPredicate::True,
            AbstractPredicate::False,
            AbstractPredicate::True,
            |_| AbstractPredicate::True,
            |p| p,
            AbstractPredicate::Or,
        )
        .await;
    }

    #[tokio::test]
    async fn basic_not() {
        let test_system = test_system();
        let TestSystem {
            system,
            dept1_id_column_path: dept1_id_column_id,
            test_system_resolver,
            ..
        } = &test_system;

        {
            // A literal

            let context = test_request_context(Value::Null, test_system_resolver); // context is irrelevant

            let scenarios = [
                (true, AbstractPredicate::False),
                (false, AbstractPredicate::True),
            ];

            for (l1, expected) in scenarios.iter() {
                let test_ae = AccessPredicateExpression::LogicalOp(AccessLogicalExpression::Not(
                    Box::new(AccessPredicateExpression::BooleanLiteral(*l1)),
                ));

                let solved_predicate = solve_access(&test_ae, &context, system).await;
                assert_eq!(&solved_predicate, expected);
            }
        }
        {
            // A context value
            let context =
                test_request_context(json!({"v1": true, "v2": false}), test_system_resolver); // context is irrelevant

            let scenarios = [
                ("v1", AbstractPredicate::False),
                ("v2", AbstractPredicate::True),
            ];

            for (c1, expected) in scenarios.iter() {
                let test_ae = AccessPredicateExpression::LogicalOp(AccessLogicalExpression::Not(
                    Box::new(boolean_context_selection(AccessContextSelection::Select(
                        Box::new(AccessContextSelection::Context("AccessContext".to_string())),
                        c1.to_string(),
                    ))),
                ));

                let solved_predicate = solve_access(&test_ae, &context, system).await;
                assert_eq!(&solved_predicate, expected);
            }
        }

        {
            // Two columns
            let test_ae = AccessPredicateExpression::LogicalOp(AccessLogicalExpression::Not(
                Box::new(boolean_column_selection(dept1_id_column_id.clone())),
            ));

            let context = test_request_context(Value::Null, test_system_resolver); // context is irrelevant
            let solved_predicate = solve_access(&test_ae, &context, system).await;
            assert_eq!(
                solved_predicate,
                AbstractPredicate::Neq(
                    test_system.dept1_id_column(),
                    ColumnPath::Literal(MaybeOwned::Owned(Box::new(true))).into()
                )
            );
        }
    }

    #[tokio::test]
    async fn context_only() {
        // Scenario: AuthContext.role == "ROLE_ADMIN"

        let TestSystem {
            system,
            test_system_resolver,
            ..
        } = test_system();

        let test_ae = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
            context_selection_expr("AccessContext", &["role"]),
            Box::new(AccessPrimitiveExpression::StringLiteral(
                "ROLE_ADMIN".to_owned(),
            )),
        ));

        let context = test_request_context(json!({"role": "ROLE_ADMIN"} ), &test_system_resolver);
        let solved_predicate = solve_access(&test_ae, &context, &system).await;
        assert_eq!(solved_predicate, AbstractPredicate::True);

        let context = test_request_context(json!({"role": "ROLE_USER"} ), &test_system_resolver);
        let solved_predicate = solve_access(&test_ae, &context, &system).await;
        assert_eq!(solved_predicate, AbstractPredicate::False);
    }

    #[tokio::test]
    async fn context_and_dynamic() {
        // Scenario: AuthContext.role == "ROLE_ADMIN" || self.published

        let test_system = test_system();
        let TestSystem {
            system,
            published_column_path,
            test_system_resolver,
            ..
        } = &test_system;

        let test_ae = {
            let admin_access = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
                context_selection_expr("AccessContext", &["role"]),
                Box::new(AccessPrimitiveExpression::StringLiteral(
                    "ROLE_ADMIN".to_owned(),
                )),
            ));
            let user_access = boolean_column_selection(published_column_path.clone());

            AccessPredicateExpression::LogicalOp(AccessLogicalExpression::Or(
                Box::new(admin_access),
                Box::new(user_access),
            ))
        };

        let context = test_request_context(json!({"role": "ROLE_ADMIN"} ), test_system_resolver);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(solved_predicate, AbstractPredicate::True);

        let context = test_request_context(json!({"role": "ROLE_USER"} ), test_system_resolver);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(
            solved_predicate,
            AbstractPredicate::Eq(
                test_system.published_column(),
                ColumnPath::Literal(MaybeOwned::Owned(Box::new(true))).into()
            )
        );
    }

    #[tokio::test]
    async fn context_compared_with_dynamic() {
        // Scenario: AuthContext.user_id == self.owner_id

        let test_system = test_system();
        let TestSystem {
            system,
            owner_id_column_path,
            test_system_resolver,
            ..
        } = &test_system;

        let test_ae = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
            context_selection_expr("AccessContext", &["user_id"]),
            Box::new(AccessPrimitiveExpression::Column(
                owner_id_column_path.clone(),
            )),
        ));

        let context = test_request_context(json!({"user_id": "1"}), test_system_resolver);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(
            solved_predicate,
            AbstractPredicate::Eq(
                test_system.owner_id_column(),
                ColumnPath::Literal(MaybeOwned::Owned(Box::new("1".to_string()))).into(),
            )
        );

        let context = test_request_context(json!({"user_id": "2"}), test_system_resolver);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(
            solved_predicate,
            AbstractPredicate::Eq(
                test_system.owner_id_column(),
                ColumnPath::Literal(MaybeOwned::Owned(Box::new("2".to_string()))).into(),
            )
        );
    }

    #[tokio::test]
    async fn varied_rule_for_roles() {
        // Scenario: AuthContext.role == "ROLE_ADMIN" || (AuthContext.role == "ROLE_USER" && self.published == true)

        let test_system = test_system();
        let TestSystem {
            system,
            published_column_path,
            test_system_resolver,
            ..
        } = &test_system;

        let admin_access = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
            context_selection_expr("AccessContext", &["role"]),
            Box::new(AccessPrimitiveExpression::StringLiteral(
                "ROLE_ADMIN".to_owned(),
            )),
        ));

        let user_access = {
            let role_rule = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
                context_selection_expr("AccessContext", &["role"]),
                Box::new(AccessPrimitiveExpression::StringLiteral(
                    "ROLE_USER".to_owned(),
                )),
            ));

            let data_rule = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
                Box::new(AccessPrimitiveExpression::Column(
                    published_column_path.clone(),
                )),
                Box::new(AccessPrimitiveExpression::BooleanLiteral(true)),
            ));

            AccessPredicateExpression::LogicalOp(AccessLogicalExpression::And(
                Box::new(role_rule),
                Box::new(data_rule),
            ))
        };

        let test_ae = AccessPredicateExpression::LogicalOp(AccessLogicalExpression::Or(
            Box::new(admin_access),
            Box::new(user_access),
        ));

        // For admins, allow access without any further restrictions
        let context = test_request_context(json!({"role": "ROLE_ADMIN"}), test_system_resolver);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(solved_predicate, AbstractPredicate::True);

        // For users, allow only if the article is published
        let context = test_request_context(json!({"role": "ROLE_USER"}), test_system_resolver);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(
            solved_predicate,
            AbstractPredicate::Eq(
                test_system.published_column(),
                ColumnPath::Literal(MaybeOwned::Owned(Box::new(true))).into(),
            )
        );

        // For other roles, do not allow
        let context = test_request_context(json!({"role": "ROLE_GUEST"}), test_system_resolver);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(solved_predicate, AbstractPredicate::False);

        // For anonymous users, too, do not allow (irrelevant context content that doesn't define a user role)
        let context = test_request_context(json!({ "Foo": "bar" }), test_system_resolver);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(solved_predicate, AbstractPredicate::False);

        // For anonymous users, too, do not allow (no context content)
        let context = test_request_context(Value::Null, test_system_resolver);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(solved_predicate, AbstractPredicate::False);
    }

    #[tokio::test]
    async fn top_level_boolean_literal() {
        let test_system = test_system();
        let TestSystem {
            test_system_resolver,
            ..
        } = &test_system;

        // Scenario: true or false
        let system = ModelPostgresSystem::default();

        let test_ae = AccessPredicateExpression::BooleanLiteral(true);
        let context = test_request_context(Value::Null, test_system_resolver); // irrelevant context content
        let solved_predicate = solve_access(&test_ae, &context, &system).await;
        assert_eq!(solved_predicate, AbstractPredicate::True);

        let test_ae = AccessPredicateExpression::BooleanLiteral(false);
        let context = test_request_context(Value::Null, test_system_resolver); // irrelevant context content
        let solved_predicate = solve_access(&test_ae, &context, &system).await;
        assert_eq!(solved_predicate, AbstractPredicate::False);
    }

    #[tokio::test]
    async fn top_level_boolean_column() {
        // Scenario: self.published

        let test_system = test_system();
        let TestSystem {
            system,
            published_column_path: published_column_id,
            test_system_resolver,
            ..
        } = &test_system;

        let test_ae = boolean_column_selection(published_column_id.clone());

        let context = test_request_context(Value::Null, test_system_resolver); // irrelevant context content
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(
            solved_predicate,
            AbstractPredicate::Eq(
                test_system.published_column(),
                ColumnPath::Literal(MaybeOwned::Owned(Box::new(true))).into()
            )
        );
    }

    #[tokio::test]
    async fn top_level_boolean_context() {
        // Scenario: AuthContext.is_admin

        let test_system = test_system();
        let TestSystem {
            system,
            test_system_resolver,
            ..
        } = &test_system;

        let test_ae = boolean_context_selection(context_selection("AccessContext", &["is_admin"]));

        let context = test_request_context(json!({"is_admin": true}), test_system_resolver);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(solved_predicate, AbstractPredicate::True);

        let context = test_request_context(json!({"is_admin": false}), test_system_resolver);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(solved_predicate, AbstractPredicate::False);

        let context = test_request_context(Value::Null, test_system_resolver); // context not provided, so we should assume that the user is not an admin
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(solved_predicate, AbstractPredicate::False);
    }

    fn test_request_context(
        test_values: Value,
        system_resolver: &SystemResolver,
    ) -> RequestContext {
        RequestContext::parse_context(
            &REQUEST,
            vec![Box::new(
                core_resolver::request_context::TestRequestContext { test_values },
            )],
            system_resolver,
        )
        .unwrap()
    }
}
