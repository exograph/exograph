// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use common::value::Val;
use core_plugin_interface::{
    core_model::access::{AccessLogicalExpression, AccessRelationalOp},
    core_resolver::access_solver::{AccessInput, AccessSolver},
};
use exo_sql::ColumnPath;
use postgres_graphql_model::subsystem::PostgresGraphQLSubsystem;

use postgres_core_model::access::PrecheckAccessPrimitiveExpression;

use std::collections::HashMap;

use core_plugin_interface::core_model::access::{
    AccessPredicateExpression, CommonAccessPrimitiveExpression, FunctionCall,
};
use exo_env::MapEnvironment;
use exo_sql::AbstractPredicate;
use serde_json::json;

use crate::access::{database_solver::literal_column, test_util::test_request_context};

use super::test_system::{context_selection_expr, router, TestSystem};

#[cfg_attr(not(target_family = "wasm"), tokio::test)]
#[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
async fn self_field_against_context() {
    // Scenario: self.age < AuthContext.id (self is a User)
    // Should leave no database residue (i.e. fully solved based on input and context)
    let test_system = TestSystem::new().await;

    let auth_context_age = || context_selection_expr("AccessContext", "id");

    let lt_expr = |expr1, expr2| {
        AccessPredicateExpression::RelationalOp(AccessRelationalOp::Lt(expr1, expr2))
    };

    let self_age = || test_system.expr("User", "age", None).into();

    let test_ae = || lt_expr(self_age(), auth_context_age());
    let test_ae_commuted = || lt_expr(auth_context_age(), self_age());

    // self.age < AuthContext.id
    let matrix = [
        // (expr, self.age, AuthContext.id, expected_result)
        (test_ae(), 1, 2, true),
        (test_ae(), 2, 1, false),
        (test_ae_commuted(), 1, 2, false),
        (test_ae_commuted(), 2, 1, true),
    ];

    for (test_ae, input_age, context_id, expected_result) in matrix {
        let context_value = json!({"id": context_id});
        let input_value = json!({"age": input_age}).into();
        let input_value = Some(AccessInput {
            value: &input_value,
            ignore_missing_value: false,
            aliases: HashMap::new(),
        });

        let solved_predicate =
            solve_access(&test_ae, context_value, input_value, &test_system.system).await;
        assert_eq!(solved_predicate, expected_result.into());
    }
}

#[cfg_attr(not(target_family = "wasm"), tokio::test)]
#[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
async fn self_field_path_static_resolve() {
    // Scenario: self.name == self.name
    let test_system = TestSystem::new().await;

    let self_name = || test_system.expr("User", "name", None).into();

    let test_ae =
        AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(self_name(), self_name()));

    let context_value = json!({});
    let input_value = json!({"name": "John"}).into();

    let input_value = Some(AccessInput {
        value: &input_value,
        ignore_missing_value: false,
        aliases: HashMap::new(),
    });
    let solved_predicate =
        solve_access(&test_ae, context_value, input_value, &test_system.system).await;
    assert_eq!(solved_predicate, true.into());
}

#[cfg_attr(not(target_family = "wasm"), tokio::test)]
#[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
async fn self_field_default_value() {
    // Scenario: self.user.id == AuthContext.id (self is a Todo)
    let test_system = TestSystem::new().await;

    let self_user_id = || test_system.expr("Todo", "user.id", None).into();

    let test_ae = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
        self_user_id(),
        context_selection_expr("AccessContext", "id"),
    ));

    let context_value = json!({"id": 1});
    let input_value =
        json!({"title": "Buy groceries", "completed": false, "user": {"id": 1}}).into();

    let input_value = Some(AccessInput {
        value: &input_value,
        ignore_missing_value: false,
        aliases: HashMap::new(),
    });
    let solved_predicate = solve_access(
        &test_ae,
        context_value.clone(),
        input_value,
        &test_system.system,
    )
    .await;
    assert_eq!(solved_predicate, true.into());

    // Use the default value for user.id (Todo has `user: User = AccessContext.id`)
    let input_value = json!({"title": "Buy groceries", "completed": false}).into();

    let input_value = Some(AccessInput {
        value: &input_value,
        ignore_missing_value: false,
        aliases: HashMap::new(),
    });

    let solved_predicate =
        solve_access(&test_ae, context_value, input_value, &test_system.system).await;
    assert_eq!(solved_predicate, true.into());
}

#[cfg_attr(not(target_family = "wasm"), tokio::test)]
#[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
async fn self_field_default_value_logical_ops() {
    // Scenario: !(self.user.id != AuthContext.id) (self is a Todo)
    // Effectively the same condition as the earlier test, but uses a logical not and inside uses != instead of ==
    let test_system = TestSystem::new().await;

    let self_user_id = || test_system.expr("Todo", "user.id", None).into();

    let test_ae = AccessPredicateExpression::LogicalOp(AccessLogicalExpression::Not(Box::new(
        AccessPredicateExpression::RelationalOp(AccessRelationalOp::Neq(
            self_user_id(),
            context_selection_expr("AccessContext", "id"),
        )),
    )));

    let context_value = json!({"id": 1});
    let input_value =
        json!({"title": "Buy groceries", "completed": false, "user": {"id": 1}}).into();

    let input_value = Some(AccessInput {
        value: &input_value,
        ignore_missing_value: false,
        aliases: HashMap::new(),
    });
    let solved_predicate = solve_access(
        &test_ae,
        context_value.clone(),
        input_value,
        &test_system.system,
    )
    .await;
    assert_eq!(solved_predicate, true.into());

    // Use the default value for user.id (Todo has `user: User = AccessContext.id`)
    let input_value = json!({"title": "Buy groceries", "completed": false}).into();

    let input_value = Some(AccessInput {
        value: &input_value,
        ignore_missing_value: false,
        aliases: HashMap::new(),
    });

    let solved_predicate =
        solve_access(&test_ae, context_value, input_value, &test_system.system).await;
    assert_eq!(solved_predicate, true.into());
}

#[cfg_attr(not(target_family = "wasm"), tokio::test)]
#[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
async fn many_to_one_pk_against_context() {
    // Scenario: self.author.id < AuthContext.id (self is an Publication)
    let test_system = TestSystem::new().await;

    let auth_context_id = || context_selection_expr("AccessContext", "id");

    let lt_expr = |expr1, expr2| {
        AccessPredicateExpression::RelationalOp(AccessRelationalOp::Lt(expr1, expr2))
    };

    let self_author_id = || test_system.expr("Publication", "author.id", None).into();

    let test_ae = || lt_expr(self_author_id(), auth_context_id());
    let test_ae_commuted = || lt_expr(auth_context_id(), self_author_id());

    // self.author.id < AuthContext.id
    let matrix = [
        // (expr, self.author.id, AuthContext.id, expected_result)
        (test_ae(), 1, 2, true),
        (test_ae(), 2, 1, false),
        (test_ae_commuted(), 1, 2, false),
        (test_ae_commuted(), 2, 1, true),
    ];

    for (test_ae, input_id, context_id, expected_result) in matrix {
        let context_value = json!({"id": context_id});
        let input_value = json!({"author": {"id": input_id}}).into();
        let input_value = Some(AccessInput {
            value: &input_value,
            ignore_missing_value: false,
            aliases: HashMap::new(),
        });

        let solved_predicate =
            solve_access(&test_ae, context_value, input_value, &test_system.system).await;
        assert_eq!(solved_predicate, expected_result.into());
    }
}

#[cfg_attr(not(target_family = "wasm"), tokio::test)]
#[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
async fn many_to_one_non_pk_field_to_against_another_non_pk_field() {
    // Scenario: self.author.name == self.author.skill (self is an Publication)
    // Example mutations:
    // createPublication(data: { ..., author: { id: 5 } })
    // updatePublication(id: 1, data: { author: { id: 5 } })
    // updatePublications(where: {...}, data: { author: { id: 5 } })
    let test_system = TestSystem::new().await;

    let publication_author_name_path = || test_system.expr("Publication", "author.name", None);
    let publication_author_skill_path = || test_system.expr("Publication", "author.skill", None);

    let eq_expr = |expr1, expr2| {
        AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
            Box::new(expr1),
            Box::new(expr2),
        ))
    };

    let test_ae = || {
        eq_expr(
            publication_author_name_path(),
            publication_author_skill_path(),
        )
    };
    let test_ae_commuted = || {
        eq_expr(
            publication_author_skill_path(),
            publication_author_name_path(),
        )
    };

    let author_id = 100;

    let relation_predicate = || {
        AbstractPredicate::Eq(
            test_system.column_path("User", "id"),
            literal_column(Val::Number(author_id.into())),
        )
    };

    let matrix = [
        (
            test_ae(),
            AbstractPredicate::and(
                AbstractPredicate::eq(
                    test_system.column_path("Publication", "author.name"),
                    test_system.column_path("Publication", "author.skill"),
                ),
                relation_predicate(),
            ),
        ),
        (
            test_ae_commuted(),
            AbstractPredicate::and(
                AbstractPredicate::eq(
                    test_system.column_path("Publication", "author.skill"),
                    test_system.column_path("Publication", "author.name"),
                ),
                relation_predicate(),
            ),
        ),
    ];

    for (test_ae, expected_result) in matrix {
        let context_value = json!({});
        let input_value = json!({"author": {"id": author_id}}).into();
        let input_value = Some(AccessInput {
            value: &input_value,
            ignore_missing_value: false,
            aliases: HashMap::new(),
        });

        let solved_predicate =
            solve_access(&test_ae, context_value, input_value, &test_system.system).await;
        assert_eq!(solved_predicate, expected_result);
    }
}

#[cfg_attr(not(target_family = "wasm"), tokio::test)]
#[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
async fn many_to_one_non_pk_match() {
    // Scenario: self.author.age < AuthContext.id (self is an Publication)
    // And input cannot provide the name (may provide only the id).
    // This should lead to a database residue.
    let test_system = TestSystem::new().await;

    // let self_author_age = || test_system.publication_author_age_expr().into();
    let self_author_age = || test_system.expr("Publication", "author.age", None).into();
    let authcontext_id = || context_selection_expr("AccessContext", "id");

    let lt_expr = |expr1, expr2| {
        AccessPredicateExpression::RelationalOp(AccessRelationalOp::Lt(expr1, expr2))
    };

    let test_ae = || lt_expr(self_author_age(), authcontext_id());
    let test_ae_commuted = || lt_expr(authcontext_id(), self_author_age());

    let age_path = test_system.column_path("User", "age");
    // Non-commuted: self.author.age < AuthContext.id
    let matrix = [
        // (expr, AuthContext.id, expected_core_predicate)
        (
            test_ae(),
            2,
            AbstractPredicate::Lt(age_path.clone(), literal_column(Val::Number(2.into()))),
        ),
        (
            test_ae(),
            1,
            AbstractPredicate::Lt(age_path.clone(), literal_column(Val::Number(1.into()))),
        ),
        (
            test_ae_commuted(),
            2,
            AbstractPredicate::Lt(literal_column(Val::Number(2.into())), age_path.clone()),
        ),
        (
            test_ae_commuted(),
            1,
            AbstractPredicate::Lt(literal_column(Val::Number(1.into())), age_path.clone()),
        ),
    ];

    for (test_ae, context_id, expected_core_predicate) in matrix {
        let context_value = json!({"id": context_id});
        let input_value = json!({"author": {"id": 100}}).into(); // We don't/can't provide the age

        let input_value = Some(AccessInput {
            value: &input_value,
            ignore_missing_value: false,
            aliases: HashMap::new(),
        });

        let solved_predicate =
            solve_access(&test_ae, context_value, input_value, &test_system.system).await;

        // The expected predicate should be the core predicate (author.age < ??) AND a predicate that specifies the value of the author's id.
        let expected_relational_predicate = AbstractPredicate::Eq(
            test_system.column_path("User", "id"),
            literal_column(Val::Number(100.into())),
        );
        assert_eq!(
            solved_predicate,
            AbstractPredicate::and(expected_core_predicate, expected_relational_predicate)
        );
    }
}

#[cfg_attr(not(target_family = "wasm"), tokio::test)]
#[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
async fn upspecifiable_field() {
    // Scenario: self.article.title == AuthContext.name
    // This is a reduced version of (self.article.publications.some(p => ...))
    // In an update mutation, the input value will not have the article field, so there is no way to evaluate the expression, so it should be ignored
    // (just like any unavalable field in an update mutation)
    //
    // Example mutation:
    // updatePublications(where: ..., data: { author: { id: 5 } })
    let test_system = TestSystem::new().await;

    let article_title_path = || test_system.expr("Article", "title", None).into();
    let auth_context_name_path = || context_selection_expr("AccessContext", "name");

    let eq_expr = |expr1, expr2| {
        AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(expr1, expr2))
    };

    let test_ae = || eq_expr(article_title_path(), auth_context_name_path());

    let context_value = || json!({"name": "John"});
    let input_value = || json!({"author": {"id": 5}}).into();

    let input_value = input_value();
    let input_value = Some(AccessInput {
        value: &input_value,
        ignore_missing_value: true,
        aliases: HashMap::new(),
    });

    let solved_predicate = solve_access(
        &test_ae(),
        context_value(),
        input_value,
        &test_system.system,
    )
    .await;

    assert_eq!(solved_predicate, true.into(),);
}

#[cfg_attr(not(target_family = "wasm"), tokio::test)]
#[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
async fn hof_call_with_equality() {
    // Scenario: self.publications.some(p => p.royalty == AuthContext.id) (where self is User)
    // This should lead to no database residue (publications and their royalty are available in the input context)
    let test_system = TestSystem::new().await;

    let function_call = |op: fn(
        Box<PrecheckAccessPrimitiveExpression>,
        Box<PrecheckAccessPrimitiveExpression>,
    ) -> AccessRelationalOp<PrecheckAccessPrimitiveExpression>| {
        PrecheckAccessPrimitiveExpression::Function(
            test_system.path("User", "publications"),
            FunctionCall {
                name: "some".to_string(),
                parameter_name: "p".to_string(),
                expr: AccessPredicateExpression::RelationalOp(op(
                    test_system.expr("Publication", "royalty", Some("p")).into(),
                    context_selection_expr("AccessContext", "id"),
                )),
            },
        )
    };

    let eq_call = || function_call(AccessRelationalOp::Eq);
    let neq_call = || function_call(AccessRelationalOp::Neq);

    let eq_call: Box<dyn Fn() -> PrecheckAccessPrimitiveExpression> = Box::new(eq_call);
    let neq_call: Box<dyn Fn() -> PrecheckAccessPrimitiveExpression> = Box::new(neq_call);

    let form_expr =
        |lhs, rhs| AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(lhs, rhs));

    let no_100 = || json!({"publications": [{"royalty": 10}, {"royalty": 20}]});
    let only_100 = || json!({"publications": [{"royalty": 100}]});
    let first_100 = || json!({"publications": [{"royalty": 100}, {"royalty": 20}]});
    let second_100 = || json!({"publications": [{"royalty": 20}, {"royalty": 100}]});
    let empty = || json!({"publications": []});

    let matrix = [
        (&eq_call, true, no_100(), false),
        (&eq_call, true, empty(), false),
        (&eq_call, true, only_100(), true),
        (&eq_call, true, first_100(), true),
        (&eq_call, true, second_100(), true),
        // With false
        (&eq_call, false, no_100(), true),
        (&eq_call, false, empty(), true),
        (&eq_call, false, only_100(), false),
        (&eq_call, false, first_100(), false),
        (&eq_call, false, second_100(), false),
        // NEQ cases
        (&neq_call, true, no_100(), true),
        (&neq_call, true, empty(), false), // some evaluation is false on an empty list
        (&neq_call, true, only_100(), false),
        (&neq_call, true, first_100(), true), // There are some non-100 articles
        (&neq_call, true, second_100(), true), // There are some non-100 articles
        // With false
        (&neq_call, false, no_100(), false),
        (&neq_call, false, empty(), true), // some evaluation is false on an empty list
        (&neq_call, false, only_100(), true),
        (&neq_call, false, first_100(), false), // There are some non-100 articles
        (&neq_call, false, second_100(), false), // There are some non-100 articles
    ];

    for (i, (lhs, rhs, input_value, expected_result)) in matrix.into_iter().enumerate() {
        let context_value = json!({"id": 100});
        let input_value = input_value.into();
        let input_value = || {
            Some(AccessInput {
                value: &input_value,
                ignore_missing_value: false,
                aliases: HashMap::new(),
            })
        };

        let boolean_expr = || {
            Box::new(PrecheckAccessPrimitiveExpression::Common(
                CommonAccessPrimitiveExpression::BooleanLiteral(rhs),
            ))
        };

        let test_ae = form_expr(Box::new(lhs()), boolean_expr());
        let expected_result = expected_result.into();

        let solved_predicate = solve_access(
            &test_ae,
            context_value.clone(),
            input_value(),
            &test_system.system,
        )
        .await;
        assert_eq!(solved_predicate, expected_result, "Test case {i}");

        let commuted_test_ae = form_expr(boolean_expr(), Box::new(lhs()));
        let solved_predicate = solve_access(
            &commuted_test_ae,
            context_value,
            input_value(),
            &test_system.system,
        )
        .await;
        assert_eq!(
            solved_predicate, expected_result,
            "Test case (commuted) {i}"
        );
    }
}

#[cfg_attr(not(target_family = "wasm"), tokio::test)]
#[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
async fn hof_no_residue() {
    // Scenario: self.publications.some(p => p.royalty == self.age) (where self is User)
    let test_system = TestSystem::new().await;

    let function_call = |op: fn(
        Box<PrecheckAccessPrimitiveExpression>,
        Box<PrecheckAccessPrimitiveExpression>,
    ) -> AccessRelationalOp<PrecheckAccessPrimitiveExpression>| {
        PrecheckAccessPrimitiveExpression::Function(
            test_system.path("User", "publications"),
            FunctionCall {
                name: "some".to_string(),
                parameter_name: "p".to_string(),
                expr: AccessPredicateExpression::RelationalOp(op(
                    test_system.expr("Publication", "royalty", Some("p")).into(),
                    test_system.expr("User", "age", None).into(),
                )),
            },
        )
    };

    let eq_call = || function_call(AccessRelationalOp::Eq);
    let neq_call = || function_call(AccessRelationalOp::Neq);

    let eq_call: Box<dyn Fn() -> PrecheckAccessPrimitiveExpression> = Box::new(eq_call);
    let neq_call: Box<dyn Fn() -> PrecheckAccessPrimitiveExpression> = Box::new(neq_call);

    let form_expr =
        |lhs, rhs| AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(lhs, rhs));

    let no_100 = || json!({"age": 100, "publications": [{"royalty": 10}, {"royalty": 20}]});
    let only_100 = || json!({"age": 100, "publications": [{"royalty": 100}]});
    let first_100 = || json!({"age": 100, "publications": [{"royalty": 100}, {"royalty": 20}]});
    let second_100 = || json!({"age": 100, "publications": [{"royalty": 20}, {"royalty": 100}]});
    let empty = || json!({"age": 100, "publications": []});

    let matrix = [
        (&eq_call, true, no_100(), false),
        (&eq_call, true, empty(), false),
        (&eq_call, true, only_100(), true),
        (&eq_call, true, first_100(), true),
        (&eq_call, true, second_100(), true),
        // With false
        (&eq_call, false, no_100(), true),
        (&eq_call, false, empty(), true),
        (&eq_call, false, only_100(), false),
        (&eq_call, false, first_100(), false),
        (&eq_call, false, second_100(), false),
        // NEQ cases
        (&neq_call, true, no_100(), true),
        (&neq_call, true, empty(), false), // some evaluation is false on an empty list
        (&neq_call, true, only_100(), false),
        (&neq_call, true, first_100(), true), // There are some non-100 articles
        (&neq_call, true, second_100(), true), // There are some non-100 articles
        // With false
        (&neq_call, false, no_100(), false),
        (&neq_call, false, empty(), true), // some evaluation is false on an empty list
        (&neq_call, false, only_100(), true),
        (&neq_call, false, first_100(), false), // There are some non-100 articles
        (&neq_call, false, second_100(), false), // There are some non-100 articles
    ];

    // Scenario: self.publications.some(p => p.royalty == self.age)
    // Example success operation (a publication's royalty matches the user age):
    // createUser(age: 100, publications: [{royalty: 100}, {royalty: 20}])
    // Example failure operation (none of the publications' royalties match the user age):
    // createUser(age: 100, publications: [{royalty: 10}, {royalty: 20}])
    for (i, (lhs, rhs, input_value, expected_result)) in matrix.into_iter().enumerate() {
        let context_value = json!({});
        let input_value = input_value.into();
        let input_value = || {
            Some(AccessInput {
                value: &input_value,
                ignore_missing_value: false,
                aliases: HashMap::new(),
            })
        };

        let rhs_expr = || {
            Box::new(PrecheckAccessPrimitiveExpression::Common(
                CommonAccessPrimitiveExpression::BooleanLiteral(rhs),
            ))
        };

        let test_ae = form_expr(Box::new(lhs()), rhs_expr());
        let expected_result = expected_result.into();

        let solved_predicate = solve_access(
            &test_ae,
            context_value.clone(),
            input_value(),
            &test_system.system,
        )
        .await;
        assert_eq!(solved_predicate, expected_result, "Test case {i}");

        let commuted_test_ae = form_expr(rhs_expr(), Box::new(lhs()));
        let solved_predicate = solve_access(
            &commuted_test_ae,
            context_value,
            input_value(),
            &test_system.system,
        )
        .await;
        assert_eq!(
            solved_predicate, expected_result,
            "Test case (commuted) {i}"
        );
    }
}

#[cfg_attr(not(target_family = "wasm"), tokio::test)]
#[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
async fn hof_with_residue() {
    // Scenario: self.article.publications.some(p => p.author.id == AuthContext.id) where self is a publication
    // Only a current author can invite (add) a new author to a publication
    // This should lead to a database residue (since publications are not in the input context)
    let test_system = TestSystem::new().await;

    let function_call = |op: fn(
        Box<PrecheckAccessPrimitiveExpression>,
        Box<PrecheckAccessPrimitiveExpression>,
    ) -> AccessRelationalOp<PrecheckAccessPrimitiveExpression>| {
        PrecheckAccessPrimitiveExpression::Function(
            test_system.path("Publication", "article.publications"),
            FunctionCall {
                name: "some".to_string(),
                parameter_name: "p".to_string(),
                expr: AccessPredicateExpression::RelationalOp(op(
                    test_system
                        .expr("Publication", "author.id", Some("p"))
                        .into(),
                    context_selection_expr("AccessContext", "id"),
                )),
            },
        )
    };

    let eq_call = || function_call(AccessRelationalOp::Eq);
    let neq_call = || function_call(AccessRelationalOp::Neq);

    let eq_call: Box<dyn Fn() -> PrecheckAccessPrimitiveExpression> = Box::new(eq_call);
    let neq_call: Box<dyn Fn() -> PrecheckAccessPrimitiveExpression> = Box::new(neq_call);

    let form_expr =
        |lhs, rhs| AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(lhs, rhs));

    let user_100 = || json!({"id": 100});

    // The input author 111 should not be a part of the predicate
    let input_value = json!({"author": {"id": 111}, "article": {"id": 50}});

    // Example: Add author 100 to article 50 (allowed if Authcontext.is is one of the authors of article 50)
    // input: createPublication(data: author: {id: 100}, article: {id: 50})
    // context: {id: 100}

    let expected_left = |op: fn(ColumnPath, ColumnPath) -> AbstractPredicate| {
        AbstractPredicate::And(
            Box::new(op(
                test_system.column_path("Article", "publications.author.id"),
                literal_column(Val::Number(100.into())),
            )),
            Box::new(AbstractPredicate::Eq(
                test_system.column_path("Article", "id"),
                literal_column(Val::Number(50.into())),
            )),
        )
    };

    let matrix = [
        (
            &eq_call,
            true,
            user_100(),
            expected_left(AbstractPredicate::Eq),
            true,
        ),
        (
            &neq_call,
            true,
            user_100(),
            expected_left(AbstractPredicate::Neq),
            true,
        ),
        (
            &eq_call,
            false,
            user_100(),
            expected_left(AbstractPredicate::Eq),
            false,
        ),
        (
            &neq_call,
            false,
            user_100(),
            expected_left(AbstractPredicate::Neq),
            false,
        ),
    ];

    for (i, (lhs, rhs, context_value, expected_left, expected_right)) in
        matrix.into_iter().enumerate()
    {
        let rhs_expr = || {
            Box::new(PrecheckAccessPrimitiveExpression::Common(
                CommonAccessPrimitiveExpression::BooleanLiteral(rhs),
            ))
        };

        let input_value = input_value.clone().into();

        let test_ae = form_expr(Box::new(lhs()), rhs_expr());

        let input_value = || {
            Some(AccessInput {
                value: &input_value,
                ignore_missing_value: false,
                aliases: HashMap::new(),
            })
        };

        let expected_result = if expected_right {
            expected_left.clone()
        } else {
            AbstractPredicate::Not(Box::new(expected_left.clone()))
        };

        let solved_predicate = solve_access(
            &test_ae,
            context_value.clone(),
            input_value(),
            &test_system.system,
        )
        .await;
        assert_eq!(solved_predicate, expected_result, "Test case {i}");

        let commuted_test_ae = form_expr(rhs_expr(), Box::new(lhs()));
        let expected_result = if expected_right {
            expected_left.clone()
        } else {
            AbstractPredicate::Not(Box::new(expected_left))
        };
        let solved_predicate = solve_access(
            &commuted_test_ae,
            context_value,
            input_value(),
            &test_system.system,
        )
        .await;
        assert_eq!(
            solved_predicate, expected_result,
            "Test case (commuted) {i}"
        );
    }
}

#[cfg_attr(not(target_family = "wasm"), tokio::test)]
#[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
async fn upspecifiable_field_with_hof() {
    // Scenario: self.article.publications.some(p => p.royalty == AuthContext.id)
    //
    // Example mutation:
    // updatePublications(where: ..., data: { author: { id: 5 } })
    let test_system = TestSystem::new().await;

    let function_call = |op: fn(
        Box<PrecheckAccessPrimitiveExpression>,
        Box<PrecheckAccessPrimitiveExpression>,
    ) -> AccessRelationalOp<PrecheckAccessPrimitiveExpression>| {
        PrecheckAccessPrimitiveExpression::Function(
            test_system.path("Publication", "article.publications"),
            FunctionCall {
                name: "some".to_string(),
                parameter_name: "p".to_string(),
                expr: AccessPredicateExpression::RelationalOp(op(
                    test_system.expr("Publication", "royalty", Some("p")).into(),
                    context_selection_expr("AccessContext", "id"),
                )),
            },
        )
    };

    let eq_call = || function_call(AccessRelationalOp::Eq);

    let context_value = || json!({"id": 100});
    let input_value = || json!({"author": {"id": 5}}).into();

    let input_value = input_value();
    let input_value = Some(AccessInput {
        value: &input_value,
        ignore_missing_value: true,
        aliases: HashMap::new(),
    });

    let form_expr =
        |lhs, rhs| AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(lhs, rhs));

    let boolean_expr = || {
        Box::new(PrecheckAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::BooleanLiteral(true),
        ))
    };

    let test_ae = form_expr(eq_call().into(), boolean_expr());

    let solved_predicate =
        solve_access(&test_ae, context_value(), input_value, &test_system.system).await;

    assert_eq!(solved_predicate, true.into(),);
}

async fn solve_access<'a>(
    expr: &'a AccessPredicateExpression<PrecheckAccessPrimitiveExpression>,
    context_value: serde_json::Value,
    input_value: Option<AccessInput<'a>>,
    subsystem: &'a PostgresGraphQLSubsystem,
) -> AbstractPredicate {
    let router = router();
    let env = &MapEnvironment::from(HashMap::new());
    let request_context = test_request_context(context_value, &router, env);

    let result = subsystem
        .solve(&request_context, input_value.as_ref(), expr)
        .await;

    match result {
        Ok(result) => result.resolve().0,
        Err(e) => panic!("Error solving access predicate: {:?}", e),
    }
}
