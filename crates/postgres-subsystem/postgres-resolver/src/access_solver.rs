// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! [`AccessSolver`] for the Postgres subsystem.
//!
//! This computes a predicate that can be either a boolean value or a residual expression that can
//! be passed down to the the underlying system (for example, a `where` clause to the database
//! query).
//!
//! This module differs from Deno/Wasm in that it has an additional primitive expression type,
//! `ColumnPath`, which we process into a predicate that we can pass to the database query.

use async_trait::async_trait;
use core_plugin_interface::{
    core_model::access::AccessRelationalOp,
    core_resolver::{
        access_solver::{
            eq_values, gt_values, gte_values, in_values, lt_values, lte_values, neq_values,
            reduce_common_primitive_expression, AccessPredicate, AccessSolver,
            SolvedCommonPrimitiveExpression,
        },
        context::RequestContext,
        value::Val,
    },
};
use exo_sql::{AbstractPredicate, ColumnPath, PhysicalColumnPath, SQLParamContainer};
use postgres_model::{
    access::{DatabaseAccessPrimitiveExpression, InputAccessPrimitiveExpression},
    subsystem::PostgresSubsystem,
};

// Only to get around the orphan rule while implementing AccessSolver
pub struct AbstractPredicateWrapper(pub AbstractPredicate);

impl std::ops::Not for AbstractPredicateWrapper {
    type Output = AbstractPredicateWrapper;

    fn not(self) -> Self::Output {
        AbstractPredicateWrapper(self.0.not())
    }
}

impl From<bool> for AbstractPredicateWrapper {
    fn from(value: bool) -> Self {
        AbstractPredicateWrapper(AbstractPredicate::from(value))
    }
}

impl<'a> AccessPredicate<'a> for AbstractPredicateWrapper {
    fn and(self, other: Self) -> Self {
        AbstractPredicateWrapper(AbstractPredicate::and(self.0, other.0))
    }

    fn or(self, other: Self) -> Self {
        AbstractPredicateWrapper(AbstractPredicate::or(self.0, other.0))
    }
}

#[derive(Debug)]
pub enum SolvedPrimitiveExpression<'a> {
    Common(SolvedCommonPrimitiveExpression<'a>),
    Column(PhysicalColumnPath),
}

#[derive(Debug)]
pub enum SolvedJsonPrimitiveExpression<'a> {
    Common(SolvedCommonPrimitiveExpression<'a>),
    Path(Vec<String>),
}

#[async_trait]
impl<'a> AccessSolver<'a, DatabaseAccessPrimitiveExpression, AbstractPredicateWrapper>
    for PostgresSubsystem
{
    async fn solve_relational_op(
        &'a self,
        request_context: &'a RequestContext<'a>,
        _input_context: Option<&'a Val>,
        op: &'a AccessRelationalOp<DatabaseAccessPrimitiveExpression>,
    ) -> AbstractPredicateWrapper {
        async fn reduce_primitive_expression<'a>(
            solver: &PostgresSubsystem,
            request_context: &'a RequestContext<'a>,
            expr: &'a DatabaseAccessPrimitiveExpression,
        ) -> SolvedPrimitiveExpression<'a> {
            match expr {
                DatabaseAccessPrimitiveExpression::Common(expr) => {
                    SolvedPrimitiveExpression::Common(
                        reduce_common_primitive_expression(solver, request_context, expr).await,
                    )
                }
                DatabaseAccessPrimitiveExpression::Column(column_path) => {
                    SolvedPrimitiveExpression::Column(column_path.clone())
                }
            }
        }

        let (left, right) = op.sides();
        let left = reduce_primitive_expression(self, request_context, left).await;
        let right = reduce_primitive_expression(self, request_context, right).await;

        type ColumnPredicateFn = fn(ColumnPath, ColumnPath) -> AbstractPredicate;
        type ValuePredicateFn = fn(Val, Val) -> AbstractPredicate;

        let helper = |unresolved_context_predicate: AbstractPredicate,
                      column_predicate: ColumnPredicateFn,
                      value_predicate: ValuePredicateFn|
         -> AbstractPredicate {
            match (left, right) {
                (
                    SolvedPrimitiveExpression::Common(
                        SolvedCommonPrimitiveExpression::UnresolvedContext(_),
                    ),
                    _,
                )
                | (
                    _,
                    SolvedPrimitiveExpression::Common(
                        SolvedCommonPrimitiveExpression::UnresolvedContext(_),
                    ),
                ) => unresolved_context_predicate,

                (
                    SolvedPrimitiveExpression::Column(left_col),
                    SolvedPrimitiveExpression::Column(right_col),
                ) => column_predicate(to_column_path(&left_col), to_column_path(&right_col)),

                (
                    SolvedPrimitiveExpression::Common(SolvedCommonPrimitiveExpression::Value(
                        left_value,
                    )),
                    SolvedPrimitiveExpression::Common(SolvedCommonPrimitiveExpression::Value(
                        right_value,
                    )),
                ) => value_predicate(left_value, right_value),

                // The next two need to be handled separately, since we need to pass the left side
                // and right side to the predicate in the correct order. For example, `age > 18` is
                // different from `18 > age`.
                (
                    SolvedPrimitiveExpression::Common(SolvedCommonPrimitiveExpression::Value(
                        value,
                    )),
                    SolvedPrimitiveExpression::Column(column),
                ) => column_predicate(literal_column(value), to_column_path(&column)),

                (
                    SolvedPrimitiveExpression::Column(column),
                    SolvedPrimitiveExpression::Common(SolvedCommonPrimitiveExpression::Value(
                        value,
                    )),
                ) => column_predicate(to_column_path(&column), literal_column(value)),
            }
        };

        AbstractPredicateWrapper(match op {
            AccessRelationalOp::Eq(..) => helper(
                AbstractPredicate::False,
                AbstractPredicate::eq,
                |left_value, right_value| eq_values(&left_value, &right_value).into(),
            ),
            AccessRelationalOp::Neq(_, _) => helper(
                // If a context is undefined, declare the expression as a match. For example,
                // `AuthContext.role != "ADMIN"` for anonymous user evaluates to true
                AbstractPredicate::True,
                AbstractPredicate::neq,
                |left_value, right_value| neq_values(&left_value, &right_value).into(),
            ),
            // For the next four, we could better optimize cases where values are comparable, but
            // for now, we generate a predicate and let database handle it
            AccessRelationalOp::Lt(_, _) => helper(
                AbstractPredicate::False,
                AbstractPredicate::Lt,
                |left_value, right_value| {
                    AbstractPredicate::Lt(literal_column(left_value), literal_column(right_value))
                },
            ),
            AccessRelationalOp::Lte(_, _) => helper(
                AbstractPredicate::False,
                AbstractPredicate::Lte,
                |left_value, right_value| {
                    AbstractPredicate::Lte(literal_column(left_value), literal_column(right_value))
                },
            ),
            AccessRelationalOp::Gt(_, _) => helper(
                AbstractPredicate::False,
                AbstractPredicate::Gt,
                |left_value, right_value| {
                    AbstractPredicate::Gt(literal_column(left_value), literal_column(right_value))
                },
            ),
            AccessRelationalOp::Gte(_, _) => helper(
                AbstractPredicate::False,
                AbstractPredicate::Gte,
                |left_value, right_value| {
                    AbstractPredicate::Gte(literal_column(left_value), literal_column(right_value))
                },
            ),
            AccessRelationalOp::In(..) => helper(
                AbstractPredicate::False,
                AbstractPredicate::In,
                |left_value, right_value| match right_value {
                    Val::List(values) => values.contains(&left_value).into(),
                    _ => unreachable!("The right side operand of `in` operator must be an array"), // This never happens see relational_op::in_relation_match
                },
            ),
        })
    }
}

#[async_trait]
impl<'a> AccessSolver<'a, InputAccessPrimitiveExpression, AbstractPredicateWrapper>
    for PostgresSubsystem
{
    async fn solve_relational_op(
        &'a self,
        request_context: &'a RequestContext<'a>,
        input_context: Option<&'a Val>,
        op: &'a AccessRelationalOp<InputAccessPrimitiveExpression>,
    ) -> AbstractPredicateWrapper {
        async fn reduce_primitive_expression<'a>(
            solver: &PostgresSubsystem,
            request_context: &'a RequestContext<'a>,
            expr: &'a InputAccessPrimitiveExpression,
        ) -> SolvedJsonPrimitiveExpression<'a> {
            match expr {
                InputAccessPrimitiveExpression::Common(expr) => {
                    SolvedJsonPrimitiveExpression::Common(
                        reduce_common_primitive_expression(solver, request_context, expr).await,
                    )
                }
                InputAccessPrimitiveExpression::Path(path) => {
                    SolvedJsonPrimitiveExpression::Path(path.clone())
                }
            }
        }

        let (left, right) = op.sides();
        let left = reduce_primitive_expression(self, request_context, left).await;
        let right = reduce_primitive_expression(self, request_context, right).await;

        type ValuePredicateFn = fn(&Val, &Val) -> bool;

        let helper =
            |unresolved_context_predicate: bool, value_predicate: ValuePredicateFn| -> bool {
                match (left, right) {
                    (
                        SolvedJsonPrimitiveExpression::Common(
                            SolvedCommonPrimitiveExpression::UnresolvedContext(_),
                        ),
                        _,
                    )
                    | (
                        _,
                        SolvedJsonPrimitiveExpression::Common(
                            SolvedCommonPrimitiveExpression::UnresolvedContext(_),
                        ),
                    ) => unresolved_context_predicate,

                    (
                        SolvedJsonPrimitiveExpression::Path(left_path),
                        SolvedJsonPrimitiveExpression::Path(right_path),
                    ) => match_paths(&left_path, &right_path, input_context, value_predicate),

                    (
                        SolvedJsonPrimitiveExpression::Common(
                            SolvedCommonPrimitiveExpression::Value(left_value),
                        ),
                        SolvedJsonPrimitiveExpression::Common(
                            SolvedCommonPrimitiveExpression::Value(right_value),
                        ),
                    ) => value_predicate(&left_value, &right_value),

                    // The next two need to be handled separately, since we need to pass the left side
                    // and right side to the predicate in the correct order. For example, `age > 18` is
                    // different from `18 > age`.
                    (
                        SolvedJsonPrimitiveExpression::Common(
                            SolvedCommonPrimitiveExpression::Value(left_value),
                        ),
                        SolvedJsonPrimitiveExpression::Path(right_path),
                    ) => {
                        let right_value = resolve_value(input_context.unwrap(), &right_path);
                        // If the user didn't provide a value, we evalute to true. Since the purpose of
                        // an input predicate is to enforce an invariant, if the user didn't provide a
                        // value, the original value will remain unchanged thus keeping the invariant
                        // intact.
                        match right_value {
                            Some(right_value) => value_predicate(&left_value, right_value),
                            None => true,
                        }
                    }

                    (
                        SolvedJsonPrimitiveExpression::Path(left_path),
                        SolvedJsonPrimitiveExpression::Common(
                            SolvedCommonPrimitiveExpression::Value(right_value),
                        ),
                    ) => {
                        let left_value = resolve_value(input_context.unwrap(), &left_path);
                        // See above
                        match left_value {
                            Some(left_value) => value_predicate(left_value, &right_value),
                            None => true,
                        }
                    }
                }
            };

        AbstractPredicateWrapper(
            match op {
                AccessRelationalOp::Eq(..) => helper(false, eq_values),
                AccessRelationalOp::Neq(_, _) => helper(
                    // If a context is undefined, declare the expression as a match. For example,
                    // `AuthContext.role != "ADMIN"` for anonymous user evaluates to true
                    true, neq_values,
                ),
                AccessRelationalOp::Lt(_, _) => helper(false, lt_values), // TODO: See issue #611
                AccessRelationalOp::Lte(_, _) => helper(false, lte_values),
                AccessRelationalOp::Gt(_, _) => helper(false, gt_values),
                AccessRelationalOp::Gte(_, _) => helper(false, gte_values),
                AccessRelationalOp::In(..) => helper(false, in_values),
            }
            .into(),
        )
    }
}

fn match_paths<'a>(
    left_path: &'a Vec<String>,
    right_path: &'a Vec<String>,
    input_context: Option<&'a Val>,
    match_values: fn(&Val, &Val) -> bool,
) -> bool {
    let left_value = resolve_value(input_context.unwrap(), left_path).unwrap();
    let right_value = resolve_value(input_context.unwrap(), right_path).unwrap();
    match_values(left_value, right_value)
}

fn resolve_value<'a>(val: &'a Val, path: &'a Vec<String>) -> Option<&'a Val> {
    let mut current = val;
    for part in path {
        match current {
            Val::Object(map) => {
                current = map.get(part)?;
            }
            _ => return None,
        }
    }
    Some(current)
}

fn to_column_path(column_id: &PhysicalColumnPath) -> ColumnPath {
    ColumnPath::Physical(column_id.clone())
}

/// Converts a value to a literal column path
fn literal_column(value: Val) -> ColumnPath {
    match value {
        Val::Null => ColumnPath::Null,
        Val::Bool(v) => ColumnPath::Param(SQLParamContainer::new(v)),
        Val::Number(v) => ColumnPath::Param(SQLParamContainer::new(v.as_i64().unwrap() as i32)), // TODO: Deal with the exact number type
        Val::String(v) => ColumnPath::Param(SQLParamContainer::new(v)),
        Val::List(values) => ColumnPath::Param(SQLParamContainer::new(
            values
                .into_iter()
                .map(|v| v.into_json().unwrap())
                .collect::<Vec<_>>(),
        )),
        Val::Object(_) | Val::Binary(_) | Val::Enum(_) => todo!(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use core_plugin_interface::{
        core_model::{
            access::{
                AccessLogicalExpression, AccessPredicateExpression, CommonAccessPrimitiveExpression,
            },
            context_type::ContextSelection,
        },
        interception::InterceptionMap,
    };

    use core_resolver::context::Request;
    use core_resolver::introspection::definition::schema::Schema;
    use core_resolver::system_resolver::SystemResolver;
    use serde_json::{json, Value};

    use super::*;

    struct TestSystem {
        system: PostgresSubsystem,
        published_column_path: PhysicalColumnPath,
        owner_id_column_path: PhysicalColumnPath,
        dept1_id_column_path: PhysicalColumnPath,
        dept2_id_column_path: PhysicalColumnPath,
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
            super::to_column_path(&self.published_column_path)
        }

        fn owner_id_column(&self) -> ColumnPath {
            super::to_column_path(&self.owner_id_column_path)
        }

        fn dept1_id_column(&self) -> ColumnPath {
            super::to_column_path(&self.dept1_id_column_path)
        }

        fn dept2_id_column(&self) -> ColumnPath {
            super::to_column_path(&self.dept2_id_column_path)
        }
    }

    async fn test_system() -> TestSystem {
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
                module ArticleModule {
                    type Article {
                        @pk id: Int = autoIncrement()
                        published: Boolean
                        @bits64 owner_id: Int 
                        @bits64 dept1_id: Int 
                        @bits64 dept2_id: Int 
                    }
                }
            "#,
            "test.exo".to_string(),
        )
        .await
        .unwrap();

        let table_id = postgres_subsystem
            .database
            .get_table_id("articles")
            .unwrap();

        let get_column_id = |column_name: &str| {
            postgres_subsystem
                .database
                .get_column_id(table_id, column_name)
                .unwrap()
        };

        let published_column_id = get_column_id("published");
        let owner_id_column_id = get_column_id("owner_id");
        let dept1_id_column_id = get_column_id("dept1_id");
        let dept2_id_column_id = get_column_id("dept2_id");

        let published_column_path = PhysicalColumnPath::leaf(published_column_id);
        let owner_id_column_path = PhysicalColumnPath::leaf(owner_id_column_id);
        let dept1_id_column_path = PhysicalColumnPath::leaf(dept1_id_column_id);
        let dept2_id_column_path = PhysicalColumnPath::leaf(dept2_id_column_id);

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
            None,
            HashMap::new(),
            10,
            10,
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

    fn context_selection(context_name: &str, path_head: &str) -> ContextSelection {
        ContextSelection {
            context_name: context_name.to_string(),
            path: (path_head.to_string(), vec![]),
        }
    }

    fn context_selection_expr(head: &str, tail: &str) -> Box<DatabaseAccessPrimitiveExpression> {
        Box::new(DatabaseAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::ContextSelection(context_selection(head, tail)),
        ))
    }

    // AuthContext.is_admin => AuthContext.is_admin == true
    fn boolean_context_selection(
        context_selection: ContextSelection,
    ) -> AccessPredicateExpression<DatabaseAccessPrimitiveExpression> {
        AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
            Box::new(DatabaseAccessPrimitiveExpression::Common(
                CommonAccessPrimitiveExpression::ContextSelection(context_selection),
            )),
            Box::new(DatabaseAccessPrimitiveExpression::Common(
                CommonAccessPrimitiveExpression::BooleanLiteral(true),
            )),
        ))
    }

    // self.published => self.published == true
    fn boolean_column_selection(
        column_path: PhysicalColumnPath,
    ) -> AccessPredicateExpression<DatabaseAccessPrimitiveExpression> {
        AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
            Box::new(DatabaseAccessPrimitiveExpression::Column(column_path)),
            Box::new(DatabaseAccessPrimitiveExpression::Common(
                CommonAccessPrimitiveExpression::BooleanLiteral(true),
            )),
        ))
    }

    async fn solve_access<'a>(
        expr: &'a AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresSubsystem,
    ) -> AbstractPredicate {
        subsystem.solve(request_context, None, expr).await.0
    }

    type CompareFn = fn(ColumnPath, ColumnPath) -> AbstractPredicate;

    async fn test_relational_op(
        test_system: &TestSystem,
        op: fn(
            Box<DatabaseAccessPrimitiveExpression>,
            Box<DatabaseAccessPrimitiveExpression>,
        ) -> AccessRelationalOp<DatabaseAccessPrimitiveExpression>,
        context_match_predicate: CompareFn,
        context_mismatch_predicate: CompareFn,
        context_missing_predicate: AbstractPredicate,
        context_value_predicate: CompareFn,
        column_column_predicate: CompareFn,
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
            &test_system().await,
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
            &test_system().await,
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
            &test_system().await,
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
            &test_system().await,
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
            &test_system().await,
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
            &test_system().await,
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
    async fn test_logical_op(
        test_system: &TestSystem,
        op: fn(
            Box<DatabaseAccessPredicateExpression>,
            Box<DatabaseAccessPredicateExpression>,
        ) -> AccessLogicalExpression<DatabaseAccessPrimitiveExpression>,
        both_value_true: AbstractPredicate,
        both_value_false: AbstractPredicate,
        one_value_true: AbstractPredicate,
        one_literal_true_other_column: fn(AbstractPredicate) -> AbstractPredicate,
        one_literal_false_other_column: fn(AbstractPredicate) -> AbstractPredicate,
        both_columns: fn(Box<AbstractPredicate>, Box<AbstractPredicate>) -> AbstractPredicate,
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
            &test_system().await,
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
            &test_system().await,
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
        let test_system = test_system().await;
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
                    Box::new(boolean_context_selection(ContextSelection {
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
        } = test_system().await;

        let test_ae = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
            context_selection_expr("AccessContext", "role"),
            Box::new(DatabaseAccessPrimitiveExpression::Common(
                CommonAccessPrimitiveExpression::StringLiteral("ROLE_ADMIN".to_owned()),
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

        let test_system = test_system().await;
        let TestSystem {
            system,
            published_column_path,
            test_system_resolver,
            ..
        } = &test_system;

        let test_ae = {
            let admin_access = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
                context_selection_expr("AccessContext", "role"),
                Box::new(DatabaseAccessPrimitiveExpression::Common(
                    CommonAccessPrimitiveExpression::StringLiteral("ROLE_ADMIN".to_owned()),
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

        let test_system = test_system().await;
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

        let test_system = test_system().await;
        let TestSystem {
            system,
            published_column_path,
            test_system_resolver,
            ..
        } = &test_system;

        let admin_access = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
            context_selection_expr("AccessContext", "role"),
            Box::new(DatabaseAccessPrimitiveExpression::Common(
                CommonAccessPrimitiveExpression::StringLiteral("ROLE_ADMIN".to_owned()),
            )),
        ));

        let user_access = {
            let role_rule = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
                context_selection_expr("AccessContext", "role"),
                Box::new(DatabaseAccessPrimitiveExpression::Common(
                    CommonAccessPrimitiveExpression::StringLiteral("ROLE_USER".to_owned()),
                )),
            ));

            let data_rule = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
                Box::new(DatabaseAccessPrimitiveExpression::Column(
                    published_column_path.clone(),
                )),
                Box::new(DatabaseAccessPrimitiveExpression::Common(
                    CommonAccessPrimitiveExpression::BooleanLiteral(true),
                )),
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
        let test_system = test_system().await;
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

        let test_system = test_system().await;
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

        let test_system = test_system().await;
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
        RequestContext::new(
            &REQUEST,
            vec![Box::new(core_resolver::context::TestRequestContext {
                test_values,
            })],
            system_resolver,
        )
        .unwrap()
    }
}
