//! [`AccessSolver`] for the Postgres subsystem.
//!
//! This computes a predicate that can be either a boolean value or a residual expression that can
//! be passed down to the the underlying system (for example, a `where` clause to the database
//! query).
//!
//! This module differs from Deno/Wasm in that it has an additional primitive expression type,
//! `ColumnPath`, which we process into a predicate that we can pass to the database query.

use crate::column_path_util;
use async_trait::async_trait;
use core_plugin_interface::{
    core_model::access::{AccessContextSelection, AccessRelationalOp},
    core_resolver::{
        access_solver::AccessPredicate, access_solver::AccessSolver,
        context_extractor::ContextExtractor, request_context::RequestContext,
    },
};
use payas_sql::{AbstractPredicate, ColumnPath, SQLParamContainer};
use postgres_model::{
    access::DatabaseAccessPrimitiveExpression, column_path::ColumnIdPath,
    subsystem::PostgresSubsystem,
};
use serde_json::Value;

// Only to get around the orphan rule while implementing AccessSolver
pub struct AbstractPredicateWrapper<'a>(pub AbstractPredicate<'a>);

impl<'a> std::ops::Not for AbstractPredicateWrapper<'a> {
    type Output = AbstractPredicateWrapper<'a>;

    fn not(self) -> Self::Output {
        AbstractPredicateWrapper(self.0.not())
    }
}

impl<'a> From<bool> for AbstractPredicateWrapper<'a> {
    fn from(value: bool) -> Self {
        AbstractPredicateWrapper(AbstractPredicate::from(value))
    }
}

impl<'a> AccessPredicate<'a> for AbstractPredicateWrapper<'a> {
    fn and(self, other: Self) -> Self {
        AbstractPredicateWrapper(AbstractPredicate::and(self.0, other.0))
    }

    fn or(self, other: Self) -> Self {
        AbstractPredicateWrapper(AbstractPredicate::or(self.0, other.0))
    }
}

#[derive(Debug)]
pub enum SolvedPrimitiveExpression<'a> {
    Value(Value),
    Column(ColumnIdPath),
    UnresolvedContext(&'a AccessContextSelection), // For example, AuthContext.role for an anonymous user
}

#[async_trait]
impl<'a> AccessSolver<'a, DatabaseAccessPrimitiveExpression, AbstractPredicateWrapper<'a>>
    for PostgresSubsystem
{
    async fn solve_relational_op(
        &'a self,
        request_context: &'a RequestContext<'a>,
        op: &'a AccessRelationalOp<DatabaseAccessPrimitiveExpression>,
    ) -> AbstractPredicateWrapper<'a> {
        async fn reduce_primitive_expression<'a>(
            solver: &PostgresSubsystem,
            request_context: &'a RequestContext<'a>,
            expr: &'a DatabaseAccessPrimitiveExpression,
        ) -> SolvedPrimitiveExpression<'a> {
            match expr {
                DatabaseAccessPrimitiveExpression::ContextSelection(selection) => solver
                    .extract_context_selection(request_context, selection)
                    .await
                    .map(SolvedPrimitiveExpression::Value)
                    .unwrap_or(SolvedPrimitiveExpression::UnresolvedContext(selection)),
                DatabaseAccessPrimitiveExpression::Column(column_path) => {
                    SolvedPrimitiveExpression::Column(column_path.clone())
                }
                DatabaseAccessPrimitiveExpression::StringLiteral(value) => {
                    SolvedPrimitiveExpression::Value(Value::String(value.clone()))
                }
                DatabaseAccessPrimitiveExpression::BooleanLiteral(value) => {
                    SolvedPrimitiveExpression::Value(Value::Bool(*value))
                }
                DatabaseAccessPrimitiveExpression::NumberLiteral(value) => {
                    SolvedPrimitiveExpression::Value(Value::Number((*value).into()))
                }
            }
        }

        let (left, right) = op.sides();
        let left = reduce_primitive_expression(self, request_context, left).await;
        let right = reduce_primitive_expression(self, request_context, right).await;

        type ColumnPredicateFn<'a> = fn(ColumnPath<'a>, ColumnPath<'a>) -> AbstractPredicate<'a>;
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
                    to_column_path(&left_col, self),
                    to_column_path(&right_col, self),
                ),

                (
                    SolvedPrimitiveExpression::Value(left_value),
                    SolvedPrimitiveExpression::Value(right_value),
                ) => value_predicate(left_value, right_value),

                // The next two need to be handled separately, since we need to pass the left side
                // and right side to the predicate in the correct order. For example, `age > 18` is
                // different from `18 > age`.
                (
                    SolvedPrimitiveExpression::Value(value),
                    SolvedPrimitiveExpression::Column(column),
                ) => column_predicate(literal_column(value), to_column_path(&column, self)),

                (
                    SolvedPrimitiveExpression::Column(column),
                    SolvedPrimitiveExpression::Value(value),
                ) => column_predicate(to_column_path(&column, self), literal_column(value)),
            }
        };

        AbstractPredicateWrapper(match op {
            AccessRelationalOp::Eq(..) => helper(
                AbstractPredicate::False,
                AbstractPredicate::eq,
                |val1, val2| (val1 == val2).into(),
            ),
            AccessRelationalOp::Neq(_, _) => helper(
                // If a context is undefined, declare the expression as a match. For example,
                // `AuthContext.role != "ADMIN"` for anonymous user evaluates to true
                AbstractPredicate::True,
                AbstractPredicate::neq,
                |val1, val2| (val1 != val2).into(),
            ),
            // For the next four, we could better optimize cases where values are comparable, but
            // for now, we generate a predicate and let database handle it
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
        })
    }
}

fn to_column_path<'a>(column_id: &ColumnIdPath, system: &'a PostgresSubsystem) -> ColumnPath<'a> {
    column_path_util::to_column_path(&Some(column_id.clone()), &None, system)
}

/// Converts a value to a literal column path
fn literal_column(value: Value) -> ColumnPath<'static> {
    match value {
        Value::Null => ColumnPath::Null,
        Value::Bool(v) => ColumnPath::Param(SQLParamContainer::new(v)),
        Value::Number(v) => ColumnPath::Param(SQLParamContainer::new(v.as_i64().unwrap() as i32)), // TODO: Deal with the exact number type
        Value::String(v) => ColumnPath::Param(SQLParamContainer::new(v)),
        Value::Array(values) => ColumnPath::Param(SQLParamContainer::new(values)),
        Value::Object(_) => todo!(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use core_plugin_interface::{
        core_model::access::{AccessLogicalExpression, AccessPredicateExpression},
        interception::InterceptionMap,
    };

    use core_resolver::introspection::definition::schema::Schema;
    use core_resolver::request_context::Request;
    use core_resolver::system_resolver::SystemResolver;
    use postgres_model::{column_id::ColumnId, column_path::ColumnIdPathLink};
    use serde_json::json;

    use super::*;

    struct TestSystem {
        system: PostgresSubsystem,
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
        fn published_column(&self) -> ColumnPath {
            super::to_column_path(&self.published_column_path, &self.system)
        }

        fn owner_id_column(&self) -> ColumnPath {
            super::to_column_path(&self.owner_id_column_path, &self.system)
        }

        fn dept1_id_column(&self) -> ColumnPath {
            super::to_column_path(&self.dept1_id_column_path, &self.system)
        }

        fn dept2_id_column(&self) -> ColumnPath {
            super::to_column_path(&self.dept2_id_column_path, &self.system)
        }
    }

    fn test_system() -> TestSystem {
        let postgres_subsystem = crate::test_utils::create_postgres_system_from_str(
            r#"
                context AccessContext {
                    @test("role") role: String 
                    @test("token1") token1: String 
                    @test("token2") token2: String 
                    @test("is_admin") is_admin: Boolean 
                    @test("user_id") user_id: String 
                    @test("v1") v1: Boolean 
                    @test("v2") v2: Boolean 
                    @test("v1_clone") v1_clone: Boolean 
                    @test("v2_clone") v2_clone: Boolean 
                }

                @postgres
                service ArticleService {
                    type Article {
                        @pk id: Int = autoIncrement()
                        published: Boolean
                        @bits(64) owner_id: Int 
                        @bits(64) dept1_id: Int 
                        @bits(64) dept2_id: Int 
                    }
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

    fn context_selection(context_name: &str, path_head: &str) -> AccessContextSelection {
        AccessContextSelection {
            context_name: context_name.to_string(),
            path: (path_head.to_string(), vec![]),
        }
    }

    fn context_selection_expr(head: &str, tail: &str) -> Box<DatabaseAccessPrimitiveExpression> {
        Box::new(DatabaseAccessPrimitiveExpression::ContextSelection(
            context_selection(head, tail),
        ))
    }

    // AuthContext.is_admin => AuthContext.is_admin == true
    fn boolean_context_selection(
        context_selection: AccessContextSelection,
    ) -> AccessPredicateExpression<DatabaseAccessPrimitiveExpression> {
        AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
            Box::new(DatabaseAccessPrimitiveExpression::ContextSelection(
                context_selection,
            )),
            Box::new(DatabaseAccessPrimitiveExpression::BooleanLiteral(true)),
        ))
    }

    // self.published => self.published == true
    fn boolean_column_selection(
        column_path: ColumnIdPath,
    ) -> AccessPredicateExpression<DatabaseAccessPrimitiveExpression> {
        AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
            Box::new(DatabaseAccessPrimitiveExpression::Column(column_path)),
            Box::new(DatabaseAccessPrimitiveExpression::BooleanLiteral(true)),
        ))
    }

    async fn solve_access<'a>(
        expr: &'a AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresSubsystem,
    ) -> AbstractPredicate<'a> {
        subsystem.solve(request_context, expr).await.0
    }

    type CompareFn<'a> = fn(ColumnPath<'a>, ColumnPath<'a>) -> AbstractPredicate<'a>;

    async fn test_relational_op<'a>(
        test_system: &'a TestSystem,
        op: fn(
            Box<DatabaseAccessPrimitiveExpression>,
            Box<DatabaseAccessPrimitiveExpression>,
        ) -> AccessRelationalOp<DatabaseAccessPrimitiveExpression>,
        context_match_predicate: CompareFn<'a>,
        context_mismatch_predicate: CompareFn<'a>,
        context_missing_predicate: AbstractPredicate<'a>,
        context_value_predicate: CompareFn<'a>,
        column_column_predicate: CompareFn<'a>,
    ) {
        let TestSystem {
            system,
            owner_id_column_path,
            dept1_id_column_path,
            dept2_id_column_path,
            test_system_resolver,
            ..
        } = &test_system;

        let relational_op = |lhs, rhs| AccessPredicateExpression::RelationalOp(op(lhs, rhs));

        macro_rules! assert_solve_access {
            ($expr:expr, $request_context:expr, $expected:expr) => {
                assert_eq!(
                    solve_access($expr, $request_context, system).await,
                    $expected
                );
            };
        }

        // Case 1: Both values from AuthContext
        {
            let test_expression = relational_op(
                context_selection_expr("AccessContext", "token1"),
                context_selection_expr("AccessContext", "token2"),
            );

            let request_context = test_request_context(
                json!({"token1": "token_value", "token2": "token_value"}),
                test_system_resolver,
            );
            let solved_predicate = solve_access(&test_expression, &request_context, system).await;
            assert_eq!(
                solved_predicate,
                context_match_predicate(
                    ColumnPath::Param(SQLParamContainer::new("token_value".to_string())),
                    ColumnPath::Param(SQLParamContainer::new("token_value".to_string())),
                )
            );

            // The mismatch case doesn't make sense for lt/lte/gt/gte, but since we don't optimize
            // (to reduce obvious matches such as 5 < 6 => Predicate::True) those cases,
            // the unoptimized predicate created works for both match and mismatch cases.

            let request_context = test_request_context(
                json!({"token1": "token_value1", "token2": "token_value2"}),
                test_system_resolver,
            );
            let solved_predicate = solve_access(&test_expression, &request_context, system).await;
            assert_eq!(
                solved_predicate,
                context_mismatch_predicate(
                    ColumnPath::Param(SQLParamContainer::new("token_value1".to_string())),
                    ColumnPath::Param(SQLParamContainer::new("token_value2".to_string())),
                )
            );
        }

        // One value from AuthContext and other from a column
        {
            let context = test_request_context(json!({"user_id": "u1"}), test_system_resolver);
            let empty_context = test_request_context(json!({}), test_system_resolver);

            {
                let test_ae = relational_op(
                    context_selection_expr("AccessContext", "user_id"),
                    Box::new(DatabaseAccessPrimitiveExpression::Column(
                        owner_id_column_path.clone(),
                    )),
                );
                assert_solve_access!(
                    &test_ae,
                    &context,
                    context_value_predicate(
                        ColumnPath::Param(SQLParamContainer::new("u1".to_string())),
                        test_system.owner_id_column(),
                    )
                );
                // No user_id, so we can definitely declare it Predicate::False
                assert_solve_access!(&test_ae, &empty_context, context_missing_predicate);
            }

            // Now with a commuted expression
            {
                let test_ae = relational_op(
                    Box::new(DatabaseAccessPrimitiveExpression::Column(
                        owner_id_column_path.clone(),
                    )),
                    context_selection_expr("AccessContext", "user_id"),
                );
                assert_solve_access!(
                    &test_ae,
                    &context,
                    context_value_predicate(
                        test_system.owner_id_column(),
                        ColumnPath::Param(SQLParamContainer::new("u1".to_string())),
                    )
                );
                // No user_id, so we can definitely declare it Predicate::False
                assert_solve_access!(&test_ae, &empty_context, context_missing_predicate);
            }
        }

        // Both values from columns
        {
            // context is irrelevant
            let request_context = test_request_context(Value::Null, test_system_resolver);

            {
                let test_ae = relational_op(
                    Box::new(DatabaseAccessPrimitiveExpression::Column(
                        dept1_id_column_path.clone(),
                    )),
                    Box::new(DatabaseAccessPrimitiveExpression::Column(
                        dept2_id_column_path.clone(),
                    )),
                );

                assert_solve_access!(
                    &test_ae,
                    &request_context,
                    column_column_predicate(
                        test_system.dept1_id_column(),
                        test_system.dept2_id_column(),
                    )
                );
            }

            // Now with a commuted expression
            {
                let test_ae = relational_op(
                    Box::new(DatabaseAccessPrimitiveExpression::Column(
                        dept2_id_column_path.clone(),
                    )),
                    Box::new(DatabaseAccessPrimitiveExpression::Column(
                        dept1_id_column_path.clone(),
                    )),
                );

                assert_solve_access!(
                    &test_ae,
                    &request_context,
                    column_column_predicate(
                        test_system.dept2_id_column(),
                        test_system.dept1_id_column(),
                    )
                );
            }
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

    type DatabaseAccessPredicateExpression =
        AccessPredicateExpression<DatabaseAccessPrimitiveExpression>;

    #[allow(clippy::too_many_arguments)]
    async fn test_logical_op<'a>(
        test_system: &'a TestSystem,
        op: fn(
            Box<DatabaseAccessPredicateExpression>,
            Box<DatabaseAccessPredicateExpression>,
        ) -> AccessLogicalExpression<DatabaseAccessPrimitiveExpression>,
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
                        c1,
                    ))),
                    Box::new(boolean_context_selection(context_selection(
                        "AccessContext",
                        c2,
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
                        ColumnPath::Param(SQLParamContainer::new(true))
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
                        ColumnPath::Param(SQLParamContainer::new(true))
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
                        ColumnPath::Param(SQLParamContainer::new(true))
                    )),
                    Box::new(AbstractPredicate::Eq(
                        test_system.dept2_id_column(),
                        ColumnPath::Param(SQLParamContainer::new(true))
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
                    Box::new(boolean_context_selection(AccessContextSelection {
                        context_name: "AccessContext".to_string(),
                        path: (c1.to_string(), vec![]),
                    })),
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
                    ColumnPath::Param(SQLParamContainer::new(true))
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
            context_selection_expr("AccessContext", "role"),
            Box::new(DatabaseAccessPrimitiveExpression::StringLiteral(
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
                context_selection_expr("AccessContext", "role"),
                Box::new(DatabaseAccessPrimitiveExpression::StringLiteral(
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
                ColumnPath::Param(SQLParamContainer::new(true))
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
            context_selection_expr("AccessContext", "user_id"),
            Box::new(DatabaseAccessPrimitiveExpression::Column(
                owner_id_column_path.clone(),
            )),
        ));

        let context = test_request_context(json!({"user_id": "1"}), test_system_resolver);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(
            solved_predicate,
            AbstractPredicate::Eq(
                ColumnPath::Param(SQLParamContainer::new("1".to_string())),
                test_system.owner_id_column(),
            )
        );

        let context = test_request_context(json!({"user_id": "2"}), test_system_resolver);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(
            solved_predicate,
            AbstractPredicate::Eq(
                ColumnPath::Param(SQLParamContainer::new("2".to_string())),
                test_system.owner_id_column(),
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
            context_selection_expr("AccessContext", "role"),
            Box::new(DatabaseAccessPrimitiveExpression::StringLiteral(
                "ROLE_ADMIN".to_owned(),
            )),
        ));

        let user_access = {
            let role_rule = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
                context_selection_expr("AccessContext", "role"),
                Box::new(DatabaseAccessPrimitiveExpression::StringLiteral(
                    "ROLE_USER".to_owned(),
                )),
            ));

            let data_rule = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
                Box::new(DatabaseAccessPrimitiveExpression::Column(
                    published_column_path.clone(),
                )),
                Box::new(DatabaseAccessPrimitiveExpression::BooleanLiteral(true)),
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
                ColumnPath::Param(SQLParamContainer::new(true)),
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
        let system = PostgresSubsystem::default();

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
                ColumnPath::Param(SQLParamContainer::new(true))
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

        let test_ae = boolean_context_selection(context_selection("AccessContext", "is_admin"));

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
