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
            reduce_common_primitive_expression, AccessPredicate, AccessSolver, AccessSolverError,
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

use crate::cast;

// Only to get around the orphan rule while implementing AccessSolver
#[derive(Debug)]
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
pub enum SolvedPrimitiveExpression {
    Common(Option<Val>),
    Column(PhysicalColumnPath),
}

#[derive(Debug)]
pub enum SolvedJsonPrimitiveExpression {
    Common(Option<Val>),
    Path(Vec<String>),
}

#[async_trait]
impl<'a> AccessSolver<'a, DatabaseAccessPrimitiveExpression, AbstractPredicateWrapper>
    for PostgresSubsystem
{
    async fn solve_relational_op(
        &self,
        request_context: &RequestContext<'a>,
        _input_context: Option<&'a Val>,
        op: &AccessRelationalOp<DatabaseAccessPrimitiveExpression>,
    ) -> Result<Option<AbstractPredicateWrapper>, AccessSolverError> {
        async fn reduce_primitive_expression<'a>(
            solver: &PostgresSubsystem,
            request_context: &'a RequestContext<'a>,
            expr: &'a DatabaseAccessPrimitiveExpression,
        ) -> Result<Option<SolvedPrimitiveExpression>, AccessSolverError> {
            Ok(match expr {
                DatabaseAccessPrimitiveExpression::Common(expr) => {
                    let primitive_expr =
                        reduce_common_primitive_expression(solver, request_context, expr).await?;
                    Some(SolvedPrimitiveExpression::Common(primitive_expr))
                }
                DatabaseAccessPrimitiveExpression::Column(column_path, _) => {
                    Some(SolvedPrimitiveExpression::Column(column_path.clone()))
                }
                DatabaseAccessPrimitiveExpression::Function(_, _) => {
                    // TODO: Fix this through better types
                    unreachable!("Function calls should not remain in the resolver expression")
                }
            })
        }

        let (left, right) = op.sides();
        let left = reduce_primitive_expression(self, request_context, left).await?;
        let right = reduce_primitive_expression(self, request_context, right).await?;

        let (left, right) = match (left, right) {
            (Some(left), Some(right)) => (left, right),
            _ => return Ok(None), // If either side is None, we can't produce a predicate
        };

        type ColumnPredicateFn = fn(ColumnPath, ColumnPath) -> AbstractPredicate;
        type ValuePredicateFn = fn(Val, Val) -> AbstractPredicate;

        let helper = |column_predicate: ColumnPredicateFn,
                      value_predicate: ValuePredicateFn|
         -> Result<Option<AbstractPredicate>, AccessSolverError> {
            match (left, right) {
                (SolvedPrimitiveExpression::Common(None), _)
                | (_, SolvedPrimitiveExpression::Common(None)) => Ok(None),

                (
                    SolvedPrimitiveExpression::Column(left_col),
                    SolvedPrimitiveExpression::Column(right_col),
                ) => Ok(Some(column_predicate(
                    to_column_path(&left_col),
                    to_column_path(&right_col),
                ))),

                (
                    SolvedPrimitiveExpression::Common(Some(left_value)),
                    SolvedPrimitiveExpression::Common(Some(right_value)),
                ) => Ok(Some(value_predicate(left_value, right_value))),

                // The next two need to be handled separately, since we need to pass the left side
                // and right side to the predicate in the correct order. For example, `age > 18` is
                // different from `18 > age`.
                (
                    SolvedPrimitiveExpression::Common(Some(value)),
                    SolvedPrimitiveExpression::Column(column),
                ) => {
                    let physical_column = column.leaf_column().get_column(&self.database);
                    Ok(Some(column_predicate(
                        cast::literal_column_path(&value, &physical_column.typ)
                            .map_err(|_| AccessSolverError::Generic("Invalid literal".into()))?,
                        to_column_path(&column),
                    )))
                }

                (
                    SolvedPrimitiveExpression::Column(column),
                    SolvedPrimitiveExpression::Common(Some(value)),
                ) => {
                    let physical_column = column.leaf_column().get_column(&self.database);

                    Ok(Some(column_predicate(
                        to_column_path(&column),
                        cast::literal_column_path(&value, &physical_column.typ)
                            .map_err(|_| AccessSolverError::Generic("Invalid literal".into()))?,
                    )))
                }
            }
        };

        let access_predicate = match op {
            AccessRelationalOp::Eq(..) => {
                helper(AbstractPredicate::eq, |left_value, right_value| {
                    eq_values(&left_value, &right_value).into()
                })
            }
            AccessRelationalOp::Neq(_, _) => {
                helper(AbstractPredicate::neq, |left_value, right_value| {
                    neq_values(&left_value, &right_value).into()
                })
            }
            // For the next four, we could optimize cases where values are comparable, but
            // for now, we generate a predicate and let the database handle it
            AccessRelationalOp::Lt(_, _) => {
                helper(AbstractPredicate::Lt, |left_value, right_value| {
                    AbstractPredicate::Lt(literal_column(left_value), literal_column(right_value))
                })
            }
            AccessRelationalOp::Lte(_, _) => {
                helper(AbstractPredicate::Lte, |left_value, right_value| {
                    AbstractPredicate::Lte(literal_column(left_value), literal_column(right_value))
                })
            }
            AccessRelationalOp::Gt(_, _) => {
                helper(AbstractPredicate::Gt, |left_value, right_value| {
                    AbstractPredicate::Gt(literal_column(left_value), literal_column(right_value))
                })
            }
            AccessRelationalOp::Gte(_, _) => {
                helper(AbstractPredicate::Gte, |left_value, right_value| {
                    AbstractPredicate::Gte(literal_column(left_value), literal_column(right_value))
                })
            }
            AccessRelationalOp::In(..) => helper(
                AbstractPredicate::In,
                |left_value, right_value| match right_value {
                    Val::List(values) => values.contains(&left_value).into(),
                    _ => unreachable!("The right side operand of `in` operator must be an array"), // This never happens see relational_op::in_relation_match
                },
            ),
        }?;

        Ok(access_predicate.map(AbstractPredicateWrapper))
    }
}

#[async_trait]
impl<'a> AccessSolver<'a, InputAccessPrimitiveExpression, AbstractPredicateWrapper>
    for PostgresSubsystem
{
    async fn solve_relational_op(
        &self,
        request_context: &RequestContext<'a>,
        input_context: Option<&'a Val>,
        op: &AccessRelationalOp<InputAccessPrimitiveExpression>,
    ) -> Result<Option<AbstractPredicateWrapper>, AccessSolverError> {
        async fn reduce_primitive_expression<'a>(
            solver: &PostgresSubsystem,
            request_context: &'a RequestContext<'a>,
            expr: &'a InputAccessPrimitiveExpression,
        ) -> Result<Option<SolvedJsonPrimitiveExpression>, AccessSolverError> {
            Ok(match expr {
                InputAccessPrimitiveExpression::Common(expr) => {
                    let primitive_expr =
                        reduce_common_primitive_expression(solver, request_context, expr).await?;
                    Some(SolvedJsonPrimitiveExpression::Common(primitive_expr))
                }
                InputAccessPrimitiveExpression::Path(path, _) => {
                    Some(SolvedJsonPrimitiveExpression::Path(path.clone()))
                }
                InputAccessPrimitiveExpression::Function(_, _) => {
                    unreachable!("Function calls should not remain in the resolver expression")
                }
            })
        }

        let (left, right) = op.sides();
        let left = reduce_primitive_expression(self, request_context, left).await?;
        let right = reduce_primitive_expression(self, request_context, right).await?;

        let (left, right) = match (left, right) {
            (Some(left), Some(right)) => (left, right),
            _ => return Ok(None), // If either side is None, we can't produce a predicate
        };

        type ValuePredicateFn = fn(&Val, &Val) -> bool;

        let helper = |value_predicate: ValuePredicateFn| -> Option<bool> {
            match (left, right) {
                (SolvedJsonPrimitiveExpression::Common(None), _)
                | (_, SolvedJsonPrimitiveExpression::Common(None)) => None,

                (
                    SolvedJsonPrimitiveExpression::Path(left_path),
                    SolvedJsonPrimitiveExpression::Path(right_path),
                ) => Some(match_paths(
                    &left_path,
                    &right_path,
                    input_context,
                    value_predicate,
                )),

                (
                    SolvedJsonPrimitiveExpression::Common(Some(left_value)),
                    SolvedJsonPrimitiveExpression::Common(Some(right_value)),
                ) => Some(value_predicate(&left_value, &right_value)),

                // The next two need to be handled separately, since we need to pass the left side
                // and right side to the predicate in the correct order. For example, `age > 18` is
                // different from `18 > age`.
                (
                    SolvedJsonPrimitiveExpression::Common(Some(left_value)),
                    SolvedJsonPrimitiveExpression::Path(right_path),
                ) => {
                    let right_value = resolve_value(input_context.unwrap(), &right_path);
                    // If the user didn't provide a value, we evaluate to true. Since the purpose of
                    // an input predicate is to enforce an invariant, if the user didn't provide a
                    // value, the original value will remain unchanged thus keeping the invariant
                    // intact.
                    match right_value {
                        Some(right_value) => Some(value_predicate(&left_value, right_value)),
                        None => Some(true),
                    }
                }

                (
                    SolvedJsonPrimitiveExpression::Path(left_path),
                    SolvedJsonPrimitiveExpression::Common(Some(right_value)),
                ) => {
                    let left_value = resolve_value(input_context.unwrap(), &left_path);
                    // See above
                    match left_value {
                        Some(left_value) => Some(value_predicate(left_value, &right_value)),
                        None => Some(true),
                    }
                }
            }
        };

        Ok(match op {
            AccessRelationalOp::Eq(..) => helper(eq_values),
            AccessRelationalOp::Neq(_, _) => helper(neq_values),
            AccessRelationalOp::Lt(_, _) => helper(lt_values),
            AccessRelationalOp::Lte(_, _) => helper(lte_values),
            AccessRelationalOp::Gt(_, _) => helper(gt_values),
            AccessRelationalOp::Gte(_, _) => helper(gte_values),
            AccessRelationalOp::In(..) => helper(in_values),
        }
        .map(|p| AbstractPredicateWrapper(p.into())))
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
        Val::Bool(v) => ColumnPath::Param(SQLParamContainer::bool(v)),
        Val::Number(v) => ColumnPath::Param(SQLParamContainer::i32(v.as_i64().unwrap() as i32)), // TODO: Deal with the exact number type
        Val::String(v) => ColumnPath::Param(SQLParamContainer::string(v)),
        Val::List(_) | Val::Object(_) | Val::Binary(_) | Val::Enum(_) => todo!(),
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
        trusted_documents::TrustedDocuments,
    };

    use core_resolver::context::Request;
    use core_resolver::introspection::definition::schema::Schema;
    use core_resolver::system_resolver::SystemResolver;
    use exo_env::MapEnvironment;
    use exo_sql::PhysicalTableName;
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
            .get_table_id(&PhysicalTableName::new("articles", None))
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
            TrustedDocuments::all(),
            Schema::new(vec![], vec![], vec![]),
            None.into(),
            Box::new(MapEnvironment::from(HashMap::new())),
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
            Box::new(DatabaseAccessPrimitiveExpression::Column(column_path, None)),
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
        subsystem
            .solve(request_context, None, expr)
            .await
            .unwrap()
            .map(|p| p.0)
            .unwrap_or(AbstractPredicate::False)
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
                    ColumnPath::Param(SQLParamContainer::string("token_value".to_string())),
                    ColumnPath::Param(SQLParamContainer::string("token_value".to_string())),
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
                    ColumnPath::Param(SQLParamContainer::string("token_value1".to_string())),
                    ColumnPath::Param(SQLParamContainer::string("token_value2".to_string())),
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
                        None,
                    )),
                );
                assert_solve_access!(
                    &test_ae,
                    &context,
                    context_value_predicate(
                        ColumnPath::Param(SQLParamContainer::string("u1".to_string())),
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
                        None,
                    )),
                    context_selection_expr("AccessContext", "user_id"),
                );
                assert_solve_access!(
                    &test_ae,
                    &context,
                    context_value_predicate(
                        test_system.owner_id_column(),
                        ColumnPath::Param(SQLParamContainer::string("u1".to_string())),
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
                        None,
                    )),
                    Box::new(DatabaseAccessPrimitiveExpression::Column(
                        dept2_id_column_path.clone(),
                        None,
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
                        None,
                    )),
                    Box::new(DatabaseAccessPrimitiveExpression::Column(
                        dept1_id_column_path.clone(),
                        None,
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

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
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

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn basic_neq() {
        test_relational_op(
            &test_system().await,
            AccessRelationalOp::Neq,
            |_, _| AbstractPredicate::False,
            |_, _| AbstractPredicate::True,
            AbstractPredicate::False,
            AbstractPredicate::Neq,
            AbstractPredicate::Neq,
        )
        .await;
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
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

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
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

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
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

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
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
                        ColumnPath::Param(SQLParamContainer::bool(true))
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
                        ColumnPath::Param(SQLParamContainer::bool(true))
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
                        ColumnPath::Param(SQLParamContainer::bool(true))
                    )),
                    Box::new(AbstractPredicate::Eq(
                        test_system.dept2_id_column(),
                        ColumnPath::Param(SQLParamContainer::bool(true))
                    ))
                )
            );
        }
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
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

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
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

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
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
                    ColumnPath::Param(SQLParamContainer::bool(true))
                )
            );
        }
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
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

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
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
                ColumnPath::Param(SQLParamContainer::bool(true))
            )
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
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
                None,
            )),
        ));

        let context = test_request_context(json!({"user_id": "1"}), test_system_resolver);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(
            solved_predicate,
            AbstractPredicate::Eq(
                ColumnPath::Param(SQLParamContainer::string("1".to_string())),
                test_system.owner_id_column(),
            )
        );

        let context = test_request_context(json!({"user_id": "2"}), test_system_resolver);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(
            solved_predicate,
            AbstractPredicate::Eq(
                ColumnPath::Param(SQLParamContainer::string("2".to_string())),
                test_system.owner_id_column(),
            )
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
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
                    None,
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
                ColumnPath::Param(SQLParamContainer::bool(true)),
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

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
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

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn top_level_boolean_column() {
        // Scenario: self.published

        let test_system = test_system().await;
        let TestSystem {
            system,
            published_column_path,
            test_system_resolver,
            ..
        } = &test_system;

        let test_ae = boolean_column_selection(published_column_path.clone());

        let context = test_request_context(Value::Null, test_system_resolver); // irrelevant context content
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(
            solved_predicate,
            AbstractPredicate::Eq(
                test_system.published_column(),
                ColumnPath::Param(SQLParamContainer::bool(true))
            )
        );
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
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

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn missing_context_independent_expressions() {
        let test_system = test_system().await;
        let TestSystem {
            system,
            owner_id_column_path,
            test_system_resolver,
            ..
        } = &test_system;

        let context = test_request_context(Value::Null, test_system_resolver); // undefined context

        fn negate(
            expr: AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
        ) -> AccessPredicateExpression<DatabaseAccessPrimitiveExpression> {
            AccessPredicateExpression::LogicalOp(AccessLogicalExpression::Not(Box::new(expr)))
        }

        // If the context is not provided, any relational operation using that context should be
        // considered false. Also, their complement should also be considered false.

        // Create a test matrix of relational operators and expressions to test the following axes:
        // - Binary operators
        // - Left expression with column, literal, and context
        // - Right expression with column, literal, and context
        // - Negation of the above
        for op in [
            AccessRelationalOp::Eq,
            AccessRelationalOp::Neq,
            AccessRelationalOp::Lt,
            AccessRelationalOp::Lte,
            AccessRelationalOp::Gt,
            AccessRelationalOp::Gte,
        ] {
            // We test with both a column and a literal on the other side of the relational operator
            let column_expr_fn =
                &|| DatabaseAccessPrimitiveExpression::Column(owner_id_column_path.clone(), None);
            let literal_expr_fn = &|| {
                DatabaseAccessPrimitiveExpression::Common(
                    CommonAccessPrimitiveExpression::NumberLiteral(1),
                )
            };
            let non_context_expr_fns: Vec<&dyn Fn() -> DatabaseAccessPrimitiveExpression> =
                vec![column_expr_fn, literal_expr_fn];

            for non_context_expr_fn in non_context_expr_fns {
                let context_exp_fn = || context_selection_expr("AccessContext", "user_id");

                // Context on the left side
                {
                    let base_expr_fn = || {
                        AccessPredicateExpression::RelationalOp(op(
                            Box::new(non_context_expr_fn()),
                            context_exp_fn(),
                        ))
                    };

                    let solved_predicate = solve_access(&base_expr_fn(), &context, system).await;
                    assert_eq!(solved_predicate, AbstractPredicate::False);

                    let solved_predicate =
                        solve_access(&negate(base_expr_fn()), &context, system).await;
                    assert_eq!(solved_predicate, AbstractPredicate::False);
                }

                // Context on the right side
                {
                    let base_expr_fn = || {
                        AccessPredicateExpression::RelationalOp(op(
                            context_exp_fn(),
                            Box::new(non_context_expr_fn()),
                        ))
                    };

                    let solved_predicate = solve_access(&base_expr_fn(), &context, system).await;
                    assert_eq!(solved_predicate, AbstractPredicate::False);

                    let solved_predicate =
                        solve_access(&negate(base_expr_fn()), &context, system).await;
                    assert_eq!(solved_predicate, AbstractPredicate::False);
                }
            }
        }
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn missing_context_expressions_with_an_or() {
        let test_system = test_system().await;
        let TestSystem {
            system,
            owner_id_column_path,
            test_system_resolver,
            ..
        } = &test_system;

        // Context isn't provided, however, `<missing-context-expression> || true` should still evaluate to true
        let test_aes = || {
            vec![
                AccessRelationalOp::Eq,
                AccessRelationalOp::Neq,
                AccessRelationalOp::Lt,
                AccessRelationalOp::Lte,
                AccessRelationalOp::Gt,
                AccessRelationalOp::Gte,
            ]
            .into_iter()
            .map(|op| {
                let base_expr_fn = || {
                    AccessPredicateExpression::LogicalOp(AccessLogicalExpression::Or(
                        Box::new(AccessPredicateExpression::RelationalOp(op(
                            context_selection_expr("AccessContext", "user_id"),
                            Box::new(DatabaseAccessPrimitiveExpression::Column(
                                owner_id_column_path.clone(),
                                None,
                            )),
                        ))),
                        Box::new(AccessPredicateExpression::BooleanLiteral(true)),
                    ))
                };
                base_expr_fn()
            })
            .collect::<Vec<_>>()
        };

        {
            let context = test_request_context(Value::Null, test_system_resolver); // undefined context

            for test_ae in test_aes() {
                let solved_predicate = solve_access(&test_ae, &context, system).await;
                assert_eq!(solved_predicate, AbstractPredicate::True);
            }
        }

        {
            // Context is defined, but the value is null
            let context = test_request_context(
                json!({"AccessContext": {"user_id": null}}),
                test_system_resolver,
            );

            for test_ae in test_aes() {
                let solved_predicate = solve_access(&test_ae, &context, system).await;
                assert_eq!(solved_predicate, AbstractPredicate::True);
            }
        }
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
    }
}
