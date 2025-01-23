// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_trait::async_trait;
use common::context::RequestContext;
use common::value::Val;
use core_plugin_interface::{
    core_model::access::{AccessRelationalOp, FunctionCall},
    core_resolver::access_solver::{
        eq_values, gt_values, gte_values, in_values, lt_values, lte_values, neq_values,
        reduce_common_primitive_expression, AccessSolver, AccessSolverError,
    },
};
use exo_sql::{
    AbstractPredicate, ColumnPath, ColumnPathLink, Database, PhysicalColumnPath, PhysicalColumnType,
};
use postgres_core_resolver::cast;
use postgres_graphql_model::subsystem::PostgresGraphQLSubsystem;

use postgres_core_model::access::{
    AccessPrimitiveExpressionPath, FieldPath, PrecheckAccessPrimitiveExpression,
};

use super::access_op::AbstractPredicateWrapper;
use super::database_solver::to_column_path;

#[derive(Debug)]
pub enum SolvedPrecheckPrimitiveExpression {
    Common(Option<Val>),
    Path(AccessPrimitiveExpressionPath),
}

#[async_trait]
impl<'a> AccessSolver<'a, PrecheckAccessPrimitiveExpression, AbstractPredicateWrapper>
    for PostgresGraphQLSubsystem
{
    async fn solve_relational_op(
        &self,
        request_context: &RequestContext<'a>,
        input_context: Option<&'a Val>,
        op: &AccessRelationalOp<PrecheckAccessPrimitiveExpression>,
    ) -> Result<Option<AbstractPredicateWrapper>, AccessSolverError> {
        async fn reduce_primitive_expression<'a>(
            solver: &PostgresGraphQLSubsystem,
            request_context: &'a RequestContext<'a>,
            input_context: Option<&'a Val>,
            expr: &'a PrecheckAccessPrimitiveExpression,
        ) -> Result<Option<SolvedPrecheckPrimitiveExpression>, AccessSolverError> {
            match expr {
                PrecheckAccessPrimitiveExpression::Common(expr) => {
                    let primitive_expr =
                        reduce_common_primitive_expression(solver, request_context, expr).await?;
                    Ok(Some(SolvedPrecheckPrimitiveExpression::Common(
                        primitive_expr,
                    )))
                }
                PrecheckAccessPrimitiveExpression::Path(path, _) => {
                    Ok(Some(SolvedPrecheckPrimitiveExpression::Path(path.clone())))
                }
                PrecheckAccessPrimitiveExpression::Function(lead, func_call) => {
                    let FunctionCall { name, expr, .. } = func_call;

                    if name != "some" {
                        return Err(AccessSolverError::Generic(
                            format!("Unsupported function: {}", name).into(),
                        ));
                    }

                    let field_path = match &lead.field_path {
                        FieldPath::Normal(field_path) => field_path,
                        FieldPath::Pk { .. } => {
                            return Err(AccessSolverError::Generic(
                                "Unexpected path leading to the `some` function".into(),
                            ));
                        }
                    };

                    let function_input_context =
                        input_context.and_then(|ctx| resolve_value(ctx, field_path));

                    match function_input_context {
                        Some(Val::List(list)) => {
                            let mut result =
                                SolvedPrecheckPrimitiveExpression::Common(Some(Val::Bool(false)));
                            for item in list {
                                let solved_expr =
                                    solver.solve(request_context, Some(item), expr).await?;

                                if let Some(AbstractPredicateWrapper(p)) = solved_expr {
                                    if p == AbstractPredicate::True {
                                        result = SolvedPrecheckPrimitiveExpression::Common(Some(
                                            Val::Bool(true),
                                        ));
                                        break;
                                    }
                                }
                            }
                            Ok(Some(result))
                        }
                        _ => Err(AccessSolverError::Generic(
                            "The path leading to the `some` function must be a list".into(),
                        )),
                    }
                }
            }
        }

        fn resolve_path<'a>(
            path: &'a AccessPrimitiveExpressionPath,
            input_context: Option<&'a Val>,
            database: &'a Database,
        ) -> Result<(Option<ColumnPath>, AbstractPredicate), AccessSolverError> {
            let column_path = &path.column_path;
            let field_path = &path.field_path;

            match &field_path {
                FieldPath::Normal(field_path) => {
                    let relational_predicate = AbstractPredicate::True;
                    let value = input_context.and_then(|ctx| resolve_value(ctx, field_path));

                    let literal_column_path =
                        compute_literal_column_path(value, column_path, database)?;

                    Ok((literal_column_path, relational_predicate))
                }
                FieldPath::Pk { lead, pk_fields } => {
                    let (head, ..) = column_path.split_head();

                    let relational_predicate = compute_relational_predicate(
                        head,
                        lead,
                        pk_fields,
                        input_context,
                        database,
                    )?;

                    Ok((
                        Some(ColumnPath::Physical(column_path.clone())),
                        relational_predicate,
                    ))
                }
            }
        }

        let (left, right) = op.sides();
        let left = reduce_primitive_expression(self, request_context, input_context, left).await?;
        let right =
            reduce_primitive_expression(self, request_context, input_context, right).await?;

        let (left, right) = match (left, right) {
            (Some(left), Some(right)) => (left, right),
            _ => return Ok(None), // If either side is None, we can't produce a predicate
        };

        type ColumnPredicateFn = fn(ColumnPath, ColumnPath) -> AbstractPredicate;
        type ValuePredicateFn = fn(&Val, &Val) -> bool;

        let helper = |column_predicate: ColumnPredicateFn,
                      value_predicate: ValuePredicateFn|
         -> Result<Option<AbstractPredicate>, AccessSolverError> {
            match (left, right) {
                (SolvedPrecheckPrimitiveExpression::Common(None), _)
                | (_, SolvedPrecheckPrimitiveExpression::Common(None)) => Ok(None),

                (
                    SolvedPrecheckPrimitiveExpression::Path(left_path),
                    SolvedPrecheckPrimitiveExpression::Path(right_path),
                ) => {
                    let (left_column_path, left_predicate) =
                        resolve_path(&left_path, input_context, &self.core_subsystem.database)?;

                    let (right_column_path, right_predicate) =
                        resolve_path(&right_path, input_context, &self.core_subsystem.database)?;

                    let core_predicate = match (left_column_path, right_column_path) {
                        (Some(left_column_path), Some(right_column_path)) => {
                            column_predicate(left_column_path, right_column_path)
                        }
                        _ => AbstractPredicate::True,
                    };
                    let relational_predicate =
                        AbstractPredicate::and(left_predicate, right_predicate);

                    Ok(Some(AbstractPredicate::and(
                        core_predicate,
                        relational_predicate,
                    )))
                }

                (
                    SolvedPrecheckPrimitiveExpression::Common(Some(left_value)),
                    SolvedPrecheckPrimitiveExpression::Common(Some(right_value)),
                ) => Ok(Some(value_predicate(&left_value, &right_value).into())),

                // The next two need to be handled separately, since we need to pass the left side
                // and right side to the predicate in the correct order. For example, `age > 18` is
                // different from `18 > age`.
                (
                    SolvedPrecheckPrimitiveExpression::Common(Some(left_value)),
                    SolvedPrecheckPrimitiveExpression::Path(right_path),
                ) => {
                    match &right_path.field_path {
                        FieldPath::Normal(field_path) => {
                            // If the user didn't provide a value, we evaluate to true. Since the purpose of
                            // an input predicate is to enforce an invariant, if the user didn't provide a
                            // value, the original value will remain unchanged thus keeping the invariant
                            // intact.
                            let right_value =
                                input_context.and_then(|ctx| resolve_value(ctx, field_path));
                            match right_value {
                                Some(right_value) => {
                                    Ok(Some(value_predicate(&left_value, right_value).into()))
                                }
                                None => Ok(Some(true.into())),
                            }
                        }
                        FieldPath::Pk { lead, pk_fields } => {
                            let (right_head, right_tail_path) = right_path.column_path.split_head();

                            let (right_column_path, left_column_path) = compute_relational_sides(
                                &right_tail_path.unwrap(),
                                &left_value,
                                &self.core_subsystem.database,
                            )?;

                            let core_predicate =
                                column_predicate(left_column_path, right_column_path);
                            let relational_predicate = compute_relational_predicate(
                                right_head,
                                lead,
                                pk_fields,
                                input_context,
                                &self.core_subsystem.database,
                            )?;

                            Ok(Some(AbstractPredicate::and(
                                core_predicate,
                                relational_predicate,
                            )))
                        }
                    }
                }

                (
                    SolvedPrecheckPrimitiveExpression::Path(left_path),
                    SolvedPrecheckPrimitiveExpression::Common(Some(right_value)),
                ) => match &left_path.field_path {
                    FieldPath::Normal(field_path) => {
                        let left_value =
                            input_context.and_then(|ctx| resolve_value(ctx, field_path));
                        match left_value {
                            Some(left_value) => {
                                Ok(Some(value_predicate(left_value, &right_value).into()))
                            }
                            None => Ok(Some(true.into())),
                        }
                    }
                    FieldPath::Pk { lead, pk_fields } => {
                        let (left_head, left_tail_path) = left_path.column_path.split_head();

                        let (left_column_path, right_column_path) = compute_relational_sides(
                            &left_tail_path.unwrap(),
                            &right_value,
                            &self.core_subsystem.database,
                        )?;

                        let core_predicate = column_predicate(left_column_path, right_column_path);
                        let relational_predicate = compute_relational_predicate(
                            left_head,
                            lead,
                            pk_fields,
                            input_context,
                            &self.core_subsystem.database,
                        )?;

                        Ok(Some(AbstractPredicate::and(
                            core_predicate,
                            relational_predicate,
                        )))
                    }
                },
            }
        };

        let access_predicate = match op {
            AccessRelationalOp::Eq(..) => {
                helper(AbstractPredicate::eq, |left_value, right_value| {
                    eq_values(left_value, right_value)
                })
            }
            AccessRelationalOp::Neq(_, _) => {
                helper(AbstractPredicate::neq, |left_value, right_value| {
                    neq_values(left_value, right_value)
                })
            }
            // For the next four, we could optimize cases where values are comparable, but
            // for now, we generate a predicate and let the database handle it
            AccessRelationalOp::Lt(_, _) => {
                helper(AbstractPredicate::Lt, |left_value, right_value| {
                    lt_values(left_value, right_value)
                })
            }
            AccessRelationalOp::Lte(_, _) => {
                helper(AbstractPredicate::Lte, |left_value, right_value| {
                    lte_values(left_value, right_value)
                })
            }
            AccessRelationalOp::Gt(_, _) => {
                helper(AbstractPredicate::Gt, |left_value, right_value| {
                    gt_values(left_value, right_value)
                })
            }
            AccessRelationalOp::Gte(_, _) => {
                helper(AbstractPredicate::Gte, |left_value, right_value| {
                    gte_values(left_value, right_value)
                })
            }
            AccessRelationalOp::In(..) => {
                helper(AbstractPredicate::In, |left_value, right_value| {
                    in_values(left_value, right_value)
                })
            }
        }?;

        Ok(access_predicate.map(AbstractPredicateWrapper))
    }
}

fn compute_relational_sides(
    tail_path: &PhysicalColumnPath,
    value: &Val,
    database: &Database,
) -> Result<(ColumnPath, ColumnPath), AccessSolverError> {
    let path_column_path = to_column_path(tail_path);

    let value_column_path =
        cast::literal_column_path(value, column_type(tail_path, database), false)
            .map_err(|_| AccessSolverError::Generic("Invalid literal".into()))?;

    Ok((path_column_path, value_column_path))
}

fn compute_literal_column_path(
    value: Option<&Val>,
    associated_column_path: &PhysicalColumnPath,
    database: &Database,
) -> Result<Option<ColumnPath>, AccessSolverError> {
    value
        .map(|v| cast::literal_column_path(v, column_type(associated_column_path, database), false))
        .transpose()
        .map_err(|_| AccessSolverError::Generic("Invalid literal".into()))
}

fn compute_relational_predicate(
    head_link: ColumnPathLink,
    lead: &Vec<String>,
    pk_fields: &Vec<String>,
    input_context: Option<&Val>,
    database: &Database,
) -> Result<AbstractPredicate, AccessSolverError> {
    let lead_value = input_context.and_then(|ctx| resolve_value(ctx, lead));

    match head_link {
        ColumnPathLink::Relation(relation) => relation.column_pairs.iter().zip(pk_fields).try_fold(
            AbstractPredicate::True,
            |acc, (pair, pk_field)| {
                let pk_field_path = vec![pk_field.clone()];
                let pk_value = lead_value.and_then(|ctx| resolve_value(ctx, &pk_field_path));

                match pk_value {
                    Some(pk_value) => {
                        let foreign_physical_column_path =
                            PhysicalColumnPath::leaf(pair.foreign_column_id);
                        let foreign_column_path =
                            ColumnPath::Physical(foreign_physical_column_path.clone());
                        let literal_column_path = compute_literal_column_path(
                            Some(pk_value),
                            &foreign_physical_column_path,
                            database,
                        )?
                        .unwrap_or(ColumnPath::Null);

                        Ok(AbstractPredicate::and(
                            acc,
                            AbstractPredicate::eq(foreign_column_path, literal_column_path),
                        ))
                    }
                    None => Err(AccessSolverError::Generic(
                        format!("Could not resolve value for primary key {}", pk_field).into(),
                    )),
                }
            },
        ),
        ColumnPathLink::Leaf(column_id) => Err(AccessSolverError::Generic(
            format!("Invalid column path: {:?}", column_id).into(),
        )),
    }
}

fn column_type<'a>(
    physical_column_path: &PhysicalColumnPath,
    database: &'a Database,
) -> &'a PhysicalColumnType {
    &physical_column_path.leaf_column().get_column(database).typ
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

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use common::router::{PlainRequestPayload, Router};
    use core_plugin_interface::core_model::access::{
        AccessPredicateExpression, CommonAccessPrimitiveExpression, FunctionCall,
    };
    use exo_env::MapEnvironment;
    use exo_sql::{
        AbstractPredicate, ColumnId, ColumnPathLink, PhysicalColumnPath, PhysicalTableName,
        RelationColumnPair, TableId,
    };
    use postgres_core_model::access::FieldPath;
    use serde_json::json;

    use crate::access::{
        database_solver::literal_column,
        test_util::{context_selection, test_request_context, TestRouter},
    };

    use super::*;

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn self_field_against_context() {
        // Scenario: self.age < AuthContext.id (self is a User)
        // Should leave no database residue (i.e. fully solved based on input and context)
        let test_system = test_system().await;
        let TestSystem {
            system,
            test_system_router,
            ..
        } = &test_system;

        let test_system_router = test_system_router.as_ref();
        let env = &MapEnvironment::from(HashMap::new());

        let auth_context_age = || context_selection_expr("AccessContext", "id");

        let lt_expr = |expr1, expr2| {
            AccessPredicateExpression::RelationalOp(AccessRelationalOp::Lt(expr1, expr2))
        };

        let self_age = || test_system.user_self_age_expr().into();

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
            let context = test_request_context(json!({"id": context_id} ), test_system_router, env);
            let input_context = Some(json!({"age": input_age}).into());

            let solved_predicate =
                solve_access(&test_ae, &context, input_context.as_ref(), system).await;
            assert_eq!(solved_predicate, expected_result.into());
        }
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn self_field_path_static_resolve() {
        // Scenario: self.name == self.name
        let test_system = test_system().await;
        let TestSystem {
            system,
            test_system_router,
            ..
        } = &test_system;

        let test_system_router = test_system_router.as_ref();
        let env = &MapEnvironment::from(HashMap::new());

        let self_name = || test_system.user_self_name_expr().into();

        let test_ae = AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
            self_name(),
            self_name(),
        ));

        let context = test_request_context(json!({}), test_system_router, env);
        let input_context = Some(json!({"name": "John"}).into());

        let solved_predicate =
            solve_access(&test_ae, &context, input_context.as_ref(), system).await;
        assert_eq!(solved_predicate, true.into());
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn many_to_one_pk_against_context() {
        // Scenario: self.author.id < AuthContext.id (self is an Article)
        let test_system = test_system().await;
        let TestSystem {
            system,
            test_system_router,
            ..
        } = &test_system;

        let test_system_router = test_system_router.as_ref();
        let env = &MapEnvironment::from(HashMap::new());

        let auth_context_id = || context_selection_expr("AccessContext", "id");

        let lt_expr = |expr1, expr2| {
            AccessPredicateExpression::RelationalOp(AccessRelationalOp::Lt(expr1, expr2))
        };

        let self_author_id = || test_system.article_self_author_id_expr().into();

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
            let context = test_request_context(json!({"id": context_id} ), test_system_router, env);
            let input_context = Some(json!({"author": {"id": input_id}}).into());

            let solved_predicate =
                solve_access(&test_ae, &context, input_context.as_ref(), system).await;
            assert_eq!(solved_predicate, expected_result.into());
        }
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn many_to_one_non_pk_field_to_against_another_non_pk_field() {
        // Scenario: self.author.name == self.author.skill (self is an Article)
        let test_system = test_system().await;
        let TestSystem {
            system,
            test_system_router,
            ..
        } = &test_system;

        let test_system_router = test_system_router.as_ref();
        let env = &MapEnvironment::from(HashMap::new());

        let article_author_name_path = || test_system.article_author_name_expr();
        let article_author_skill_path = || test_system.article_author_skill_expr();

        let eq_expr = |expr1, expr2| {
            AccessPredicateExpression::RelationalOp(AccessRelationalOp::Eq(
                Box::new(expr1),
                Box::new(expr2),
            ))
        };

        let test_ae = || eq_expr(article_author_name_path(), article_author_skill_path());
        let test_ae_commuted = || eq_expr(article_author_skill_path(), article_author_name_path());

        let author_id = 100;

        let relation_predicate = || {
            AbstractPredicate::Eq(
                to_column_path(&test_system.user_id_column_path()),
                literal_column(Val::Number(author_id.into())),
            )
        };

        let matrix = [
            (
                test_ae(),
                AbstractPredicate::and(
                    AbstractPredicate::eq(
                        to_column_path(&test_system.article_author_name_physical_column_path()),
                        to_column_path(&test_system.article_author_skill_physical_column_path()),
                    ),
                    relation_predicate(),
                ),
            ),
            (
                test_ae_commuted(),
                AbstractPredicate::and(
                    AbstractPredicate::eq(
                        to_column_path(&test_system.article_author_skill_physical_column_path()),
                        to_column_path(&test_system.article_author_name_physical_column_path()),
                    ),
                    relation_predicate(),
                ),
            ),
        ];

        for (test_ae, expected_result) in matrix {
            let context = test_request_context(json!({}), test_system_router, env);
            let input_context = Some(json!({"author": {"id": author_id}}).into());

            let solved_predicate =
                solve_access(&test_ae, &context, input_context.as_ref(), system).await;
            assert_eq!(solved_predicate, expected_result);
        }
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn many_to_one_non_pk_match() {
        // Scenario: self.author.age < AuthContext.id (self is an Article)
        // And input cannot provide the name (may provide only the id).
        // This should lead to a database residue.
        let test_system = test_system().await;
        let TestSystem {
            system,
            test_system_router,
            ..
        } = &test_system;

        let test_system_router = test_system_router.as_ref();
        let env = &MapEnvironment::from(HashMap::new());

        let self_author_age = || test_system.article_author_age_expr().into();

        let authcontext_id = || context_selection_expr("AccessContext", "id");

        let lt_expr = |expr1, expr2| {
            AccessPredicateExpression::RelationalOp(AccessRelationalOp::Lt(expr1, expr2))
        };

        let test_ae = || lt_expr(self_author_age(), authcontext_id());
        let test_ae_commuted = || lt_expr(authcontext_id(), self_author_age());

        let age_path = to_column_path(&test_system.user_age_column_path());

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
            let context = test_request_context(json!({"id": context_id} ), test_system_router, env);
            let input_context = Some(json!({"author": {"id": 100}}).into()); // We don't/can't provide the age

            let solved_predicate =
                solve_access(&test_ae, &context, input_context.as_ref(), system).await;

            // The expected predicate should be the core predicate (author.age < ??) AND a predicate that specifies the value of the author's id.
            let expected_relational_predicate = AbstractPredicate::Eq(
                to_column_path(&test_system.user_id_column_path()),
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
    async fn hof_call_with_equality() {
        // Scenario: self.articles.some(i => i.title == AuthContext.name)
        let test_system = test_system().await;
        let TestSystem {
            system,
            test_system_router,
            ..
        } = &test_system;

        let test_system_router = test_system_router.as_ref();
        let env = &MapEnvironment::from(HashMap::new());

        let function_call = |op: fn(
            Box<PrecheckAccessPrimitiveExpression>,
            Box<PrecheckAccessPrimitiveExpression>,
        )
            -> AccessRelationalOp<PrecheckAccessPrimitiveExpression>| {
            PrecheckAccessPrimitiveExpression::Function(
                test_system.user_articles_path(),
                FunctionCall {
                    name: "some".to_string(),
                    parameter_name: "i".to_string(),
                    expr: AccessPredicateExpression::RelationalOp(op(
                        test_system.article_title_expr().into(),
                        context_selection_expr("AccessContext", "name"),
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

        let no_john = || json!({"articles": [{"title": "Article 1"}, {"title": "Article 2"}]});
        let only_john = || json!({"articles": [{"title": "John"}]});
        let first_john = || json!({"articles": [{"title": "John"}, {"title": "Article 2"}]});
        let second_john = || json!({"articles": [{"title": "Article 1"}, {"title": "John"}]});
        let empty = || json!({"articles": []});

        let matrix = [
            (&eq_call, true, no_john(), false),
            (&eq_call, true, empty(), false),
            (&eq_call, true, only_john(), true),
            (&eq_call, true, first_john(), true),
            (&eq_call, true, second_john(), true),
            // With false
            (&eq_call, false, no_john(), true),
            (&eq_call, false, empty(), true),
            (&eq_call, false, only_john(), false),
            (&eq_call, false, first_john(), false),
            (&eq_call, false, second_john(), false),
            // NEQ cases
            (&neq_call, true, no_john(), true),
            (&neq_call, true, empty(), false), // some evaluation is false on an empty list
            (&neq_call, true, only_john(), false),
            (&neq_call, true, first_john(), true), // There are some non-john articles
            (&neq_call, true, second_john(), true), // There are some non-john articles
            // With false
            (&neq_call, false, no_john(), false),
            (&neq_call, false, empty(), true), // some evaluation is false on an empty list
            (&neq_call, false, only_john(), true),
            (&neq_call, false, first_john(), false), // There are some non-john articles
            (&neq_call, false, second_john(), false), // There are some non-john articles
        ];

        for (i, (lhs, rhs, input_context, expected_result)) in matrix.into_iter().enumerate() {
            let context = test_request_context(json!({"name": "John"}), test_system_router, env);
            let input_context = Some(input_context.into());

            let boolean_expr = || {
                Box::new(PrecheckAccessPrimitiveExpression::Common(
                    CommonAccessPrimitiveExpression::BooleanLiteral(rhs),
                ))
            };

            let test_ae = form_expr(Box::new(lhs()), boolean_expr());
            let expected_result = expected_result.into();

            let solved_predicate =
                solve_access(&test_ae, &context, input_context.as_ref(), system).await;
            assert_eq!(solved_predicate, expected_result, "Test case {i}");

            let commuted_test_ae = form_expr(boolean_expr(), Box::new(lhs()));
            let solved_predicate =
                solve_access(&commuted_test_ae, &context, input_context.as_ref(), system).await;
            assert_eq!(
                solved_predicate, expected_result,
                "Test case (commuted) {i}"
            );
        }
    }

    async fn solve_access<'a>(
        expr: &'a AccessPredicateExpression<PrecheckAccessPrimitiveExpression>,
        request_context: &'a RequestContext<'a>,
        input_context: Option<&'a Val>,
        subsystem: &'a PostgresGraphQLSubsystem,
    ) -> AbstractPredicate {
        subsystem
            .solve(request_context, input_context, expr)
            .await
            .unwrap()
            .map(|p| p.0)
            .unwrap_or(AbstractPredicate::False)
    }

    #[allow(dead_code)]
    struct TestSystem {
        system: PostgresGraphQLSubsystem,

        article_table_id: TableId,
        article_title_column_id: ColumnId,
        article_author_column_id: ColumnId,

        user_table_id: TableId,
        user_id_column_id: ColumnId,
        user_name_column_id: ColumnId,
        user_skill_column_id: ColumnId,
        user_age_column_id: ColumnId,

        test_system_router:
            Box<dyn for<'request> Router<PlainRequestPayload<'request>> + Send + Sync>,
    }

    impl TestSystem {
        pub fn article_title_column_path(&self) -> PhysicalColumnPath {
            PhysicalColumnPath::leaf(self.article_title_column_id)
        }

        // self.title for `Article`
        pub fn article_title_expr(&self) -> PrecheckAccessPrimitiveExpression {
            PrecheckAccessPrimitiveExpression::Path(
                AccessPrimitiveExpressionPath {
                    column_path: self.article_title_column_path(),
                    field_path: FieldPath::Normal(vec!["title".to_string()]),
                },
                None,
            )
        }

        pub fn article_author_column_path(&self) -> PhysicalColumnPath {
            PhysicalColumnPath::leaf(self.article_author_column_id)
        }

        pub fn article_author_link(&self) -> ColumnPathLink {
            ColumnPathLink::relation(
                vec![RelationColumnPair {
                    self_column_id: self.article_author_column_id,
                    foreign_column_id: self.user_id_column_id,
                }],
                Some("author".to_string()),
            )
        }

        pub fn article_author_name_physical_column_path(&self) -> PhysicalColumnPath {
            let path = PhysicalColumnPath::init(self.article_author_link());
            path.push(ColumnPathLink::Leaf(self.user_name_column_id))
        }

        pub fn article_author_skill_physical_column_path(&self) -> PhysicalColumnPath {
            let path = PhysicalColumnPath::init(self.article_author_link());
            path.push(ColumnPathLink::Leaf(self.user_skill_column_id))
        }

        pub fn article_author_age_physical_column_path(&self) -> PhysicalColumnPath {
            let path = PhysicalColumnPath::init(self.article_author_link());
            path.push(ColumnPathLink::Leaf(self.user_age_column_id))
        }

        // self.author.id for `Article`
        pub fn article_self_author_id_expr(&self) -> PrecheckAccessPrimitiveExpression {
            PrecheckAccessPrimitiveExpression::Path(
                AccessPrimitiveExpressionPath {
                    column_path: self.article_author_column_path(),
                    field_path: FieldPath::Normal(vec!["author".to_string(), "id".to_string()]),
                },
                None,
            )
        }

        pub fn article_author_name_expr(&self) -> PrecheckAccessPrimitiveExpression {
            PrecheckAccessPrimitiveExpression::Path(
                AccessPrimitiveExpressionPath {
                    column_path: self.article_author_name_physical_column_path(),
                    field_path: FieldPath::Pk {
                        lead: vec!["author".to_string()],
                        pk_fields: vec!["id".to_string()],
                    },
                },
                None,
            )
        }

        pub fn article_author_skill_expr(&self) -> PrecheckAccessPrimitiveExpression {
            PrecheckAccessPrimitiveExpression::Path(
                AccessPrimitiveExpressionPath {
                    column_path: self.article_author_skill_physical_column_path(),
                    field_path: FieldPath::Pk {
                        lead: vec!["author".to_string()],
                        pk_fields: vec!["id".to_string()],
                    },
                },
                None,
            )
        }

        pub fn article_author_age_expr(&self) -> PrecheckAccessPrimitiveExpression {
            PrecheckAccessPrimitiveExpression::Path(
                AccessPrimitiveExpressionPath {
                    column_path: self.article_author_age_physical_column_path(),
                    field_path: FieldPath::Pk {
                        lead: vec!["author".to_string()],
                        pk_fields: vec!["id".to_string()],
                    },
                },
                None,
            )
        }

        pub fn user_id_column_path(&self) -> PhysicalColumnPath {
            PhysicalColumnPath::leaf(self.user_id_column_id)
        }

        pub fn user_name_column_path(&self) -> PhysicalColumnPath {
            PhysicalColumnPath::leaf(self.user_name_column_id)
        }

        pub fn user_age_column_path(&self) -> PhysicalColumnPath {
            PhysicalColumnPath::leaf(self.user_age_column_id)
        }

        pub fn user_self_age_expr(&self) -> PrecheckAccessPrimitiveExpression {
            PrecheckAccessPrimitiveExpression::Path(
                AccessPrimitiveExpressionPath {
                    column_path: self.user_age_column_path(),
                    field_path: FieldPath::Normal(vec!["age".to_string()]),
                },
                None,
            )
        }

        pub fn user_self_name_expr(&self) -> PrecheckAccessPrimitiveExpression {
            PrecheckAccessPrimitiveExpression::Path(
                AccessPrimitiveExpressionPath {
                    column_path: self.user_name_column_path(),
                    field_path: FieldPath::Normal(vec!["name".to_string()]),
                },
                None,
            )
        }

        pub fn user_articles_link(&self) -> ColumnPathLink {
            ColumnPathLink::relation(
                vec![RelationColumnPair {
                    self_column_id: self.user_id_column_id,
                    foreign_column_id: self.article_author_column_id,
                }],
                Some("articles".to_string()),
            )
        }

        pub fn user_articles_path(&self) -> AccessPrimitiveExpressionPath {
            AccessPrimitiveExpressionPath {
                column_path: PhysicalColumnPath::init(self.user_articles_link()),
                field_path: FieldPath::Normal(vec!["articles".to_string()]),
            }
        }

        #[allow(dead_code)]
        pub fn user_articles_title_physical_column_path(&self) -> PhysicalColumnPath {
            let path = PhysicalColumnPath::init(self.user_articles_link());
            path.push(ColumnPathLink::Leaf(self.article_title_column_id))
        }

        #[allow(dead_code)]
        pub fn user_articles_title_expr(&self) -> PrecheckAccessPrimitiveExpression {
            PrecheckAccessPrimitiveExpression::Path(
                AccessPrimitiveExpressionPath {
                    column_path: self.user_articles_title_physical_column_path(),
                    field_path: FieldPath::Normal(vec![
                        "articles".to_string(),
                        "title".to_string(),
                    ]),
                },
                None,
            )
        }
    }

    async fn test_system() -> TestSystem {
        let postgres_subsystem = crate::test_utils::create_postgres_system_from_str(
            r#"
                context AccessContext {
                    @test("role") role: String
                    @test("name") name: String
                    @test("id") id: Int
                }

                @postgres
                module ArticleModule {
                    type Article {
                        @pk id: Int = autoIncrement()
                        title: String
                        author: User
                    }

                    type User {
                        @pk id: Int = autoIncrement()
                        name: String
                        skill: String
                        age: Int
                        articles: Set<Article>?
                    }
                }
            "#,
            "index.exo".to_string(),
        )
        .await
        .unwrap();

        let database = &postgres_subsystem.core_subsystem.database;

        let article_table_id = database
            .get_table_id(&PhysicalTableName::new("articles", None))
            .unwrap();

        let article_title_column_id = database.get_column_id(article_table_id, "title").unwrap();
        let article_author_column_id = database
            .get_column_id(article_table_id, "author_id")
            .unwrap();

        let user_table_id = database
            .get_table_id(&PhysicalTableName::new("users", None))
            .unwrap();

        let user_id_column_id = database.get_column_id(user_table_id, "id").unwrap();

        let user_name_column_id = database.get_column_id(user_table_id, "name").unwrap();
        let user_age_column_id = database.get_column_id(user_table_id, "age").unwrap();

        let user_skill_column_id = database.get_column_id(user_table_id, "skill").unwrap();

        // Create an empty Router. Since in tests we never invoke it (since we don't have @query context),
        // we don't need to populate it.
        let test_system_router = Box::new(TestRouter {});

        TestSystem {
            system: postgres_subsystem,
            article_table_id,
            article_title_column_id,
            article_author_column_id,
            user_table_id,
            user_id_column_id,
            user_name_column_id,
            user_skill_column_id,
            user_age_column_id,
            test_system_router,
        }
    }

    fn context_selection_expr(head: &str, tail: &str) -> Box<PrecheckAccessPrimitiveExpression> {
        Box::new(PrecheckAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::ContextSelection(context_selection(head, tail)),
        ))
    }
}
