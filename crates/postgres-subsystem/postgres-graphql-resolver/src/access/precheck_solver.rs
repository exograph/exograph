// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use async_trait::async_trait;
use common::context::RequestContext;
use common::value::Val;
use core_plugin_interface::{
    core_model::access::{AccessRelationalOp, FunctionCall},
    core_resolver::access_solver::{
        eq_values, gt_values, gte_values, in_values, lt_values, lte_values, neq_values,
        reduce_common_primitive_expression, AccessInputContext, AccessInputContextPath,
        AccessInputContextPathElement, AccessSolver, AccessSolverError,
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
        input_context: Option<&AccessInputContext<'a>>,
        op: &AccessRelationalOp<PrecheckAccessPrimitiveExpression>,
    ) -> Result<Option<AbstractPredicateWrapper>, AccessSolverError> {
        async fn reduce_primitive_expression<'a>(
            solver: &PostgresGraphQLSubsystem,
            request_context: &'a RequestContext<'a>,
            input_context: Option<&AccessInputContext<'a>>,
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
                PrecheckAccessPrimitiveExpression::Path(path, parameter_name) => {
                    let mut path_elements = match parameter_name {
                        Some(parameter_name) => {
                            vec![AccessInputContextPathElement::Property(parameter_name)]
                        }
                        None => vec![],
                    };
                    let field_path_strings = match &path.field_path {
                        FieldPath::Normal(field_path) => field_path,
                        FieldPath::Pk { .. } => {
                            return Ok(Some(SolvedPrecheckPrimitiveExpression::Path(path.clone())));
                        }
                    };
                    path_elements.extend(
                        field_path_strings
                            .iter()
                            .map(|s| AccessInputContextPathElement::Property(s.as_str())),
                    );
                    let value =
                        input_context.map(|ctx| ctx.resolve(AccessInputContextPath(path_elements)));

                    let value = value.transpose()?.flatten();

                    match value {
                        Some(value) => Ok(Some(SolvedPrecheckPrimitiveExpression::Common(Some(
                            value.clone(),
                        )))),
                        None => Ok(Some(SolvedPrecheckPrimitiveExpression::Path(path.clone()))),
                    }
                }
                PrecheckAccessPrimitiveExpression::Function(lead, func_call) => {
                    let FunctionCall {
                        name,
                        parameter_name,
                        expr,
                    } = func_call;

                    if name != "some" {
                        return Err(AccessSolverError::Generic(
                            format!("Unsupported function: {}", name).into(),
                        ));
                    }

                    let field_path = match &lead.field_path {
                        FieldPath::Normal(field_path) => field_path,
                        FieldPath::Pk { .. } => {
                            // return Ok(Some(SolvedPrecheckPrimitiveExpression::Path(
                            //     AccessPrimitiveExpressionPath {
                            //         column_path: lead
                            //             .clone()
                            //             .column_path
                            //             .join(func_call.expr.column_path),
                            //         field_path: lead.field_path.clone(),
                            //     },
                            // )));
                            return Err(AccessSolverError::Generic(
                                "Unexpected path leading to the `some` function".into(),
                            ));
                        }
                    };

                    let function_input_value: Option<Result<Option<&Val>, _>> =
                        input_context.as_ref().map(|ctx| {
                            ctx.resolve(AccessInputContextPath(
                                field_path
                                    .iter()
                                    .map(|s| AccessInputContextPathElement::Property(s.as_str()))
                                    .collect(),
                            ))
                        });

                    let function_input_value = function_input_value.transpose()?.flatten();

                    match function_input_value {
                        Some(Val::List(list)) => {
                            let mut result =
                                SolvedPrecheckPrimitiveExpression::Common(Some(Val::Bool(false)));
                            for index in 0..list.len() {
                                let item_input_path = {
                                    let mut item_input_path_elements: Vec<_> = field_path
                                        .iter()
                                        .map(|s| {
                                            AccessInputContextPathElement::Property(s.as_str())
                                        })
                                        .collect();
                                    item_input_path_elements
                                        .push(AccessInputContextPathElement::Index(index));
                                    AccessInputContextPath(item_input_path_elements)
                                };

                                let new_input_context =
                                    input_context.map(|ctx| AccessInputContext {
                                        value: ctx.value,
                                        ignore_missing_context: ctx.ignore_missing_context,
                                        aliases: HashMap::from([(
                                            parameter_name.as_str(),
                                            item_input_path,
                                        )]),
                                    });

                                let solved_expr = solver
                                    .solve(request_context, new_input_context.as_ref(), expr)
                                    .await?;

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
                        _ => {
                            let ignore_missing_context = input_context
                                .as_ref()
                                .map(|ctx| ctx.ignore_missing_context)
                                .unwrap_or(true);

                            if ignore_missing_context {
                                Ok(Some(SolvedPrecheckPrimitiveExpression::Common(Some(
                                    Val::Bool(true),
                                ))))
                            } else {
                                Err(AccessSolverError::Generic(
                                    "Could not evaluate the access condition".into(),
                                ))
                            }
                        }
                    }
                }
            }
        }

        fn resolve_path<'a>(
            path: &'a AccessPrimitiveExpressionPath,
            input_context: Option<&AccessInputContext<'a>>,
            database: &'a Database,
        ) -> Result<(Option<ColumnPath>, AbstractPredicate), AccessSolverError> {
            let column_path = &path.column_path;
            let field_path = &path.field_path;

            match &field_path {
                FieldPath::Normal(field_path) => {
                    let relational_predicate = AbstractPredicate::True;

                    let value = resolve_value(input_context, field_path)?;

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
                            let right_value = resolve_value(input_context, field_path)?;
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
                        let left_value = resolve_value(input_context, field_path)?;
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
    lead: &[String],
    pk_fields: &[String],
    input_context: Option<&AccessInputContext<'_>>,
    database: &Database,
) -> Result<AbstractPredicate, AccessSolverError> {
    let lead_value = resolve_value(input_context, lead)?;

    match head_link {
        ColumnPathLink::Relation(relation) => relation.column_pairs.iter().zip(pk_fields).try_fold(
            AbstractPredicate::True,
            |acc, (pair, pk_field)| {
                let pk_field_path = vec![pk_field.clone()];
                let pk_value = lead_value.and_then(|lead_value| {
                    resolve_value(
                        Some(&AccessInputContext {
                            value: lead_value,
                            ignore_missing_context: false,
                            aliases: input_context
                                .map(|ctx| ctx.aliases.clone())
                                .unwrap_or_default(),
                        }),
                        &pk_field_path,
                    )
                    .unwrap()
                });

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

fn resolve_value<'a>(
    input_context: Option<&AccessInputContext<'a>>,
    path: &'a [String],
) -> Result<Option<&'a Val>, AccessSolverError> {
    let value: Option<Result<Option<&Val>, _>> = input_context.as_ref().map(|ctx| {
        ctx.resolve(AccessInputContextPath(
            path.iter()
                .map(|s| AccessInputContextPathElement::Property(s.as_str()))
                .collect(),
        ))
    });

    let value = value.transpose()?.flatten();

    Ok(value)
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use core_plugin_interface::core_model::access::{
        AccessPredicateExpression, CommonAccessPrimitiveExpression, FunctionCall,
    };
    use exo_env::MapEnvironment;
    use exo_sql::AbstractPredicate;
    use serde_json::json;

    use crate::access::{
        article_user_test_system::TestSystem,
        database_solver::literal_column,
        test_util::{context_selection, test_request_context},
    };

    use super::*;

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn self_field_against_context() {
        // Scenario: self.age < AuthContext.id (self is a User)
        // Should leave no database residue (i.e. fully solved based on input and context)
        let test_system = TestSystem::new().await;
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
            let input_value = json!({"age": input_age}).into();
            let input_context = Some(AccessInputContext {
                value: &input_value,
                ignore_missing_context: false,
                aliases: HashMap::new(),
            });

            let solved_predicate =
                solve_access(&test_ae, &context, input_context.as_ref(), system).await;
            assert_eq!(solved_predicate, expected_result.into());
        }
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn self_field_path_static_resolve() {
        // Scenario: self.name == self.name
        let test_system = TestSystem::new().await;
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
        let input_context = json!({"name": "John"}).into();

        let input_context = AccessInputContext {
            value: &input_context,
            ignore_missing_context: false,
            aliases: HashMap::new(),
        };
        let solved_predicate = solve_access(&test_ae, &context, Some(&input_context), system).await;
        assert_eq!(solved_predicate, true.into());
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn many_to_one_pk_against_context() {
        // Scenario: self.author.id < AuthContext.id (self is an Publication)
        let test_system = TestSystem::new().await;
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

        let self_author_id = || test_system.publication_author_id_expr().into();

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
            let input_value = json!({"author": {"id": input_id}}).into();
            let input_context = Some(AccessInputContext {
                value: &input_value,
                ignore_missing_context: false,
                aliases: HashMap::new(),
            });

            let solved_predicate =
                solve_access(&test_ae, &context, input_context.as_ref(), system).await;
            assert_eq!(solved_predicate, expected_result.into());
        }
    }

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn many_to_one_non_pk_field_to_against_another_non_pk_field() {
        // Scenario: self.author.name == self.author.skill (self is an Publication)
        let test_system = TestSystem::new().await;
        let TestSystem {
            system,
            test_system_router,
            ..
        } = &test_system;

        let test_system_router = test_system_router.as_ref();
        let env = &MapEnvironment::from(HashMap::new());

        let publication_author_name_path = || test_system.publication_author_name_expr();
        let publication_author_skill_path = || test_system.publication_author_skill_expr();

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
                to_column_path(&test_system.user_id_column_path()),
                literal_column(Val::Number(author_id.into())),
            )
        };

        let matrix = [
            (
                test_ae(),
                AbstractPredicate::and(
                    AbstractPredicate::eq(
                        to_column_path(&test_system.publication_author_name_physical_column_path()),
                        to_column_path(
                            &test_system.publication_author_skill_physical_column_path(),
                        ),
                    ),
                    relation_predicate(),
                ),
            ),
            (
                test_ae_commuted(),
                AbstractPredicate::and(
                    AbstractPredicate::eq(
                        to_column_path(
                            &test_system.publication_author_skill_physical_column_path(),
                        ),
                        to_column_path(&test_system.publication_author_name_physical_column_path()),
                    ),
                    relation_predicate(),
                ),
            ),
        ];

        for (test_ae, expected_result) in matrix {
            let context = test_request_context(json!({}), test_system_router, env);
            let input_value = json!({"author": {"id": author_id}}).into();
            let input_context = Some(AccessInputContext {
                value: &input_value,
                ignore_missing_context: false,
                aliases: HashMap::new(),
            });

            let solved_predicate =
                solve_access(&test_ae, &context, input_context.as_ref(), system).await;
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
        let TestSystem {
            system,
            test_system_router,
            ..
        } = &test_system;

        let test_system_router = test_system_router.as_ref();
        let env = &MapEnvironment::from(HashMap::new());

        let self_author_age = || test_system.publication_author_age_expr().into();

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
            let input_value = json!({"author": {"id": 100}}).into(); // We don't/can't provide the age

            let input_context = Some(AccessInputContext {
                value: &input_value,
                ignore_missing_context: false,
                aliases: HashMap::new(),
            });

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
        // Scenario: self.publications.some(p => p.royalty == AuthContext.id) (where self is User)
        // This should lead to no database residue (publications and their royalty are available in the input context)
        let test_system = TestSystem::new().await;
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
                test_system.user_publications_path(),
                FunctionCall {
                    name: "some".to_string(),
                    parameter_name: "p".to_string(),
                    expr: AccessPredicateExpression::RelationalOp(op(
                        test_system
                            .publication_royalty_expr(Some("p".to_string()))
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
            let context = test_request_context(json!({"id": 100}), test_system_router, env);
            let input_value = input_value.into();
            let input_context = Some(AccessInputContext {
                value: &input_value,
                ignore_missing_context: false,
                aliases: HashMap::new(),
            });

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

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn hof_call_no_residue() {
        // Scenario: self.publications.some(p => p.royalty == self.age) (where self is User)
        let test_system = TestSystem::new().await;
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
                test_system.user_publications_path(),
                FunctionCall {
                    name: "some".to_string(),
                    parameter_name: "p".to_string(),
                    expr: AccessPredicateExpression::RelationalOp(op(
                        test_system
                            .publication_royalty_expr(Some("p".to_string()))
                            .into(),
                        test_system.user_self_age_expr().into(),
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
        let second_100 =
            || json!({"age": 100, "publications": [{"royalty": 20}, {"royalty": 100}]});
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
            let context = test_request_context(json!({}), test_system_router, env);
            let input_value = input_value.into();
            let input_context = Some(AccessInputContext {
                value: &input_value,
                ignore_missing_context: false,
                aliases: HashMap::new(),
            });

            let rhs_expr = || {
                Box::new(PrecheckAccessPrimitiveExpression::Common(
                    CommonAccessPrimitiveExpression::BooleanLiteral(rhs),
                ))
            };

            let test_ae = form_expr(Box::new(lhs()), rhs_expr());
            let expected_result = expected_result.into();

            let solved_predicate =
                solve_access(&test_ae, &context, input_context.as_ref(), system).await;
            assert_eq!(solved_predicate, expected_result, "Test case {i}");

            let commuted_test_ae = form_expr(rhs_expr(), Box::new(lhs()));
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
        input_context: Option<&AccessInputContext<'a>>,
        subsystem: &'a PostgresGraphQLSubsystem,
    ) -> AbstractPredicate {
        let result = subsystem.solve(request_context, input_context, expr).await;

        match result {
            Ok(Some(value)) => value.0,
            Ok(None) => AbstractPredicate::False,
            Err(e) => panic!("Error solving access predicate: {:?}", e),
        }
    }

    fn context_selection_expr(head: &str, tail: &str) -> Box<PrecheckAccessPrimitiveExpression> {
        Box::new(PrecheckAccessPrimitiveExpression::Common(
            CommonAccessPrimitiveExpression::ContextSelection(context_selection(head, tail)),
        ))
    }
}
