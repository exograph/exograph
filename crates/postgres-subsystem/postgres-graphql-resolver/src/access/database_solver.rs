#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use core_plugin_interface::core_model::{
        access::{
            AccessLogicalExpression, AccessPredicateExpression, AccessRelationalOp,
            CommonAccessPrimitiveExpression,
        },
        context_type::ContextSelection,
    };

    use common::{
        context::RequestContext,
        router::{PlainRequestPayload, Router},
    };

    use exo_env::MapEnvironment;
    use exo_sql::{
        AbstractPredicate, ColumnPath, PhysicalColumnPath, PhysicalTableName, SQLParamContainer,
    };
    use postgres_core_model::access::DatabaseAccessPrimitiveExpression;
    use postgres_graphql_model::subsystem::PostgresGraphQLSubsystem;
    use serde_json::{json, Value};

    use crate::access::test_util::{context_selection, test_request_context, TestRouter};

    use core_plugin_interface::core_resolver::access_solver::AccessSolver;
    use postgres_core_resolver::access::database_solver::to_column_path;

    struct TestSystem {
        system: PostgresGraphQLSubsystem,
        published_column_path: PhysicalColumnPath,
        owner_id_column_path: PhysicalColumnPath,
        dept1_id_column_path: PhysicalColumnPath,
        dept2_id_column_path: PhysicalColumnPath,
        test_system_router:
            Box<dyn for<'request> Router<PlainRequestPayload<'request>> + Send + Sync>,
    }

    impl TestSystem {
        fn published_column(&self) -> ColumnPath {
            to_column_path(&self.published_column_path)
        }

        fn owner_id_column(&self) -> ColumnPath {
            to_column_path(&self.owner_id_column_path)
        }

        fn dept1_id_column(&self) -> ColumnPath {
            to_column_path(&self.dept1_id_column_path)
        }

        fn dept2_id_column(&self) -> ColumnPath {
            to_column_path(&self.dept2_id_column_path)
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

        let database = &postgres_subsystem.core_subsystem.database;

        let article_table_id = database
            .get_table_id(&PhysicalTableName::new("articles", None))
            .unwrap();

        let get_column_id = |column_name: &str| {
            database
                .get_column_id(article_table_id, column_name)
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

        // Create an empty Router. Since in tests we never invoke it (since we don't have @query context),
        // we don't need to populate it.
        let test_system_router = Box::new(TestRouter {});

        TestSystem {
            system: postgres_subsystem,
            published_column_path,
            owner_id_column_path,
            dept1_id_column_path,
            dept2_id_column_path,
            test_system_router,
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
        subsystem: &'a PostgresGraphQLSubsystem,
    ) -> AbstractPredicate {
        subsystem
            .core_subsystem
            .solve(request_context, None, expr)
            .await
            .unwrap()
            .map(|p| p.0)
            .resolve()
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
            test_system_router,
            ..
        } = &test_system;

        let test_system_router = test_system_router.as_ref();
        let env = &MapEnvironment::from(HashMap::new());

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
                test_system_router,
                env,
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
                test_system_router,
                env,
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
            let context = test_request_context(json!({"user_id": "u1"}), test_system_router, env);
            let empty_context = test_request_context(json!({}), test_system_router, env);

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
            let request_context = test_request_context(Value::Null, test_system_router, env);

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
            test_system_router,
            ..
        } = &test_system;

        let test_system_router = test_system_router.as_ref();
        let env = &MapEnvironment::from(HashMap::new());
        {
            // Two literals
            // context is irrelevant
            let context = test_request_context(Value::Null, test_system_router, env);

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
                test_system_router,
                env,
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
            let context = test_request_context(Value::Null, test_system_router, env); // context is irrelevant

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

            let context = test_request_context(Value::Null, test_system_router, env); // context is irrelevant
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
            test_system_router,
            ..
        } = &test_system;

        let test_system_router = test_system_router.as_ref();
        let env = &MapEnvironment::from(HashMap::new());

        {
            // A literal

            let context = test_request_context(Value::Null, test_system_router, env); // context is irrelevant

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
                test_request_context(json!({"v1": true, "v2": false}), test_system_router, env); // context is irrelevant

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

            let context = test_request_context(Value::Null, test_system_router, env); // context is irrelevant
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
            test_system_router,
            ..
        } = test_system().await;

        let test_system_router = test_system_router.as_ref();
        let env = &MapEnvironment::from(HashMap::new());
        let test_ae = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
            context_selection_expr("AccessContext", "role"),
            Box::new(DatabaseAccessPrimitiveExpression::Common(
                CommonAccessPrimitiveExpression::StringLiteral("ROLE_ADMIN".to_owned()),
            )),
        ));

        let context = test_request_context(json!({"role": "ROLE_ADMIN"} ), test_system_router, env);
        let solved_predicate = solve_access(&test_ae, &context, &system).await;
        assert_eq!(solved_predicate, AbstractPredicate::True);

        let context = test_request_context(json!({"role": "ROLE_USER"} ), test_system_router, env);
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
            test_system_router,
            ..
        } = &test_system;

        let test_system_router = test_system_router.as_ref();
        let env = &MapEnvironment::from(HashMap::new());

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

        let context = test_request_context(json!({"role": "ROLE_ADMIN"} ), test_system_router, env);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(solved_predicate, AbstractPredicate::True);

        let context = test_request_context(json!({"role": "ROLE_USER"} ), test_system_router, env);
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
            test_system_router,
            ..
        } = &test_system;

        let test_system_router = test_system_router.as_ref();
        let env = &MapEnvironment::from(HashMap::new());
        let test_ae = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
            context_selection_expr("AccessContext", "user_id"),
            Box::new(DatabaseAccessPrimitiveExpression::Column(
                owner_id_column_path.clone(),
                None,
            )),
        ));

        let context = test_request_context(json!({"user_id": "1"}), test_system_router, env);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(
            solved_predicate,
            AbstractPredicate::Eq(
                ColumnPath::Param(SQLParamContainer::string("1".to_string())),
                test_system.owner_id_column(),
            )
        );

        let context = test_request_context(json!({"user_id": "2"}), test_system_router, env);
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
            test_system_router,
            ..
        } = &test_system;

        let test_system_router = test_system_router.as_ref();
        let env = &MapEnvironment::from(HashMap::new());
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
        let context = test_request_context(json!({"role": "ROLE_ADMIN"}), test_system_router, env);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(solved_predicate, AbstractPredicate::True);

        // For users, allow only if the article is published
        let context = test_request_context(json!({"role": "ROLE_USER"}), test_system_router, env);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(
            solved_predicate,
            AbstractPredicate::Eq(
                test_system.published_column(),
                ColumnPath::Param(SQLParamContainer::bool(true)),
            )
        );

        // For other roles, do not allow
        let context = test_request_context(json!({"role": "ROLE_GUEST"}), test_system_router, env);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(solved_predicate, AbstractPredicate::False);

        // For anonymous users, too, do not allow (irrelevant context content that doesn't define a user role)
        let context = test_request_context(json!({ "Foo": "bar" }), test_system_router, env);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(solved_predicate, AbstractPredicate::False);

        // For anonymous users, too, do not allow (no context content)
        let context = test_request_context(Value::Null, test_system_router, env);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(solved_predicate, AbstractPredicate::False);
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn top_level_boolean_literal() {
        let test_system = test_system().await;
        let TestSystem {
            test_system_router, ..
        } = &test_system;

        let test_system_router = test_system_router.as_ref();
        let env = &MapEnvironment::from(HashMap::new());
        // Scenario: true or false
        let system = PostgresGraphQLSubsystem::default();

        let test_ae = AccessPredicateExpression::BooleanLiteral(true);
        let context = test_request_context(Value::Null, test_system_router, env); // irrelevant context content
        let solved_predicate = solve_access(&test_ae, &context, &system).await;
        assert_eq!(solved_predicate, AbstractPredicate::True);

        let test_ae = AccessPredicateExpression::BooleanLiteral(false);
        let context = test_request_context(Value::Null, test_system_router, env); // irrelevant context content
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
            test_system_router,
            ..
        } = &test_system;

        let test_system_router = test_system_router.as_ref();
        let env = &MapEnvironment::from(HashMap::new());
        let test_ae = boolean_column_selection(published_column_path.clone());

        let context = test_request_context(Value::Null, test_system_router, env); // irrelevant context content
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
            test_system_router,
            ..
        } = &test_system;

        let test_system_router = test_system_router.as_ref();
        let env = &MapEnvironment::from(HashMap::new());
        let test_ae = boolean_context_selection(context_selection("AccessContext", "is_admin"));

        let context = test_request_context(json!({"is_admin": true}), test_system_router, env);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(solved_predicate, AbstractPredicate::True);

        let context = test_request_context(json!({"is_admin": false}), test_system_router, env);
        let solved_predicate = solve_access(&test_ae, &context, system).await;
        assert_eq!(solved_predicate, AbstractPredicate::False);

        let context = test_request_context(Value::Null, test_system_router, env); // context not provided, so we should assume that the user is not an admin
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
            test_system_router,
            ..
        } = &test_system;

        let test_system_router = test_system_router.as_ref();
        let env = &MapEnvironment::from(HashMap::new());

        let context = test_request_context(Value::Null, test_system_router, env); // undefined context

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
            test_system_router,
            ..
        } = test_system;

        let test_system_router = test_system_router.as_ref();
        let env = &MapEnvironment::from(HashMap::new());
        let system = &system;

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
            let context = test_request_context(Value::Null, test_system_router, env); // undefined context

            for test_ae in test_aes() {
                let solved_predicate = solve_access(&test_ae, &context, system).await;
                assert_eq!(solved_predicate, AbstractPredicate::True);
            }
        }

        {
            // Context is defined, but the value is null
            let context = test_request_context(
                json!({"AccessContext": {"user_id": null}}),
                test_system_router,
                env,
            );

            for test_ae in test_aes() {
                let solved_predicate = solve_access(&test_ae, &context, system).await;
                assert_eq!(solved_predicate, AbstractPredicate::True);
            }
        }
    }
}
