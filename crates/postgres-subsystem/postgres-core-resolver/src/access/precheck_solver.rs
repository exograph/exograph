// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use crate::cast::{self, literal_column_path};
use async_trait::async_trait;
use common::context::RequestContext;
use common::value::Val;

use core_model::access::{
    AccessLogicalExpression, AccessPredicateExpression, AccessRelationalOp, FunctionCall,
};
use core_resolver::access_solver::{
    AccessInput, AccessInputPath, AccessInputPathElement, AccessSolution, AccessSolver,
    AccessSolverError, eq_values, gt_values, gte_values, in_values, lt_values, lte_values,
    neq_values, reduce_common_primitive_expression,
};
use exo_sql::{
    AbstractPredicate, BooleanColumnType, ColumnPath, ColumnPathLink, Database, PhysicalColumnPath,
    PhysicalColumnType, PhysicalColumnTypeExt,
};
use maybe_owned::MaybeOwned;
use postgres_core_model::subsystem::PostgresCoreSubsystem;

use postgres_core_model::{
    access::{AccessPrimitiveExpressionPath, FieldPath, PrecheckAccessPrimitiveExpression},
    types::PostgresFieldDefaultValue,
};

use super::access_op::AbstractPredicateWrapper;
use super::database_solver::to_column_path;

#[derive(Debug)]
enum SolvedPrecheckPrimitiveExpression {
    Common(Option<Val>),
    Path(AccessPrimitiveExpressionPath, Option<String>),
    Predicate(AbstractPredicate),
}

type ColumnPredicateFn = fn(ColumnPath, ColumnPath) -> AbstractPredicate;
type ValuePredicateFn = fn(&Val, &Val) -> bool;

#[async_trait]
impl<'a> AccessSolver<'a, PrecheckAccessPrimitiveExpression, AbstractPredicateWrapper>
    for PostgresCoreSubsystem
{
    async fn solve_relational_op(
        &self,
        request_context: &RequestContext<'a>,
        input_value: Option<&AccessInput<'a>>,
        op: &AccessRelationalOp<PrecheckAccessPrimitiveExpression>,
    ) -> Result<AccessSolution<AbstractPredicateWrapper>, AccessSolverError> {
        let (left, right) = op.sides();
        let left = reduce_primitive_expression(self, request_context, input_value, left).await?;
        let right = reduce_primitive_expression(self, request_context, input_value, right).await?;

        let (left, right) = match (left, right) {
            (AccessSolution::Solved(left), AccessSolution::Solved(right)) => (left, right),
            _ => {
                return Ok(AccessSolution::Unsolvable(AbstractPredicateWrapper(
                    AbstractPredicate::True,
                )));
            } // If either side is None, we can't produce a predicate
        };

        let ignore_missing_value = input_value
            .as_ref()
            .map(|ctx| ctx.ignore_missing_value)
            .unwrap_or(false);

        let helper = |column_predicate: ColumnPredicateFn, value_predicate: ValuePredicateFn| {
            evaluate_relation(
                self,
                request_context,
                left,
                right,
                input_value,
                &self.database,
                ignore_missing_value,
                column_predicate,
                value_predicate,
            )
        };

        let access_predicate = match op {
            AccessRelationalOp::Eq(..) => {
                helper(AbstractPredicate::eq, |left_value, right_value| {
                    eq_values(left_value, right_value)
                })
                .await
            }
            AccessRelationalOp::Neq(_, _) => {
                helper(AbstractPredicate::neq, |left_value, right_value| {
                    neq_values(left_value, right_value)
                })
                .await
            }
            // For the next four, we could optimize cases where values are comparable, but
            // for now, we generate a predicate and let the database handle it
            AccessRelationalOp::Lt(_, _) => {
                helper(AbstractPredicate::Lt, |left_value, right_value| {
                    lt_values(left_value, right_value)
                })
                .await
            }
            AccessRelationalOp::Lte(_, _) => {
                helper(AbstractPredicate::Lte, |left_value, right_value| {
                    lte_values(left_value, right_value)
                })
                .await
            }
            AccessRelationalOp::Gt(_, _) => {
                helper(AbstractPredicate::Gt, |left_value, right_value| {
                    gt_values(left_value, right_value)
                })
                .await
            }
            AccessRelationalOp::Gte(_, _) => {
                helper(AbstractPredicate::Gte, |left_value, right_value| {
                    gte_values(left_value, right_value)
                })
                .await
            }
            AccessRelationalOp::In(..) => {
                helper(AbstractPredicate::In, |left_value, right_value| {
                    in_values(left_value, right_value)
                })
                .await
            }
        }?;

        Ok(access_predicate)
    }
}

async fn reduce_primitive_expression<'a>(
    solver: &PostgresCoreSubsystem,
    request_context: &'a RequestContext<'a>,
    input_value: Option<&AccessInput<'a>>,
    expr: &'a PrecheckAccessPrimitiveExpression,
) -> Result<AccessSolution<SolvedPrecheckPrimitiveExpression>, AccessSolverError> {
    match expr {
        PrecheckAccessPrimitiveExpression::Common(expr) => {
            let primitive_expr =
                reduce_common_primitive_expression(solver, request_context, expr).await?;
            Ok(AccessSolution::Solved(
                SolvedPrecheckPrimitiveExpression::Common(primitive_expr),
            ))
        }
        PrecheckAccessPrimitiveExpression::Path(path, parameter_name) => {
            let mut path_elements = match parameter_name {
                Some(parameter_name) => {
                    vec![AccessInputPathElement::Property(parameter_name)]
                }
                None => vec![],
            };
            let field_path_strings = match &path.field_path {
                FieldPath::Normal(field_path, _) => field_path,
                FieldPath::Pk { .. } => {
                    return Ok(AccessSolution::Solved(
                        SolvedPrecheckPrimitiveExpression::Path(
                            path.clone(),
                            parameter_name.clone(),
                        ),
                    ));
                }
            };
            path_elements.extend(
                field_path_strings
                    .iter()
                    .map(|s| AccessInputPathElement::Property(s.as_str())),
            );

            let value = input_value.map(|ctx| ctx.resolve(AccessInputPath(path_elements)));

            let value = value.transpose()?.flatten();

            match value {
                Some(value) => Ok(AccessSolution::Solved(
                    SolvedPrecheckPrimitiveExpression::Common(Some(value.clone())),
                )),
                None => Ok(AccessSolution::Solved(
                    SolvedPrecheckPrimitiveExpression::Path(path.clone(), parameter_name.clone()),
                )),
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
                FieldPath::Normal(field_path, _) => field_path,
                FieldPath::Pk {
                    lead: lead_path,
                    pk_fields,
                    lead_default,
                } => {
                    let (head, tail) = lead.column_path.split_head();

                    let lead_value = resolve_value(
                        solver,
                        input_value,
                        lead_path,
                        lead_default,
                        request_context,
                    )
                    .await?;

                    // If the lead value itself is unknown, return the value of `ignore_missing_value`.
                    // See the `upspecifiable_field_with_hof` test for more details

                    let ignore_missing_value = input_value
                        .as_ref()
                        .map(|ctx| ctx.ignore_missing_value)
                        .unwrap_or(false);

                    if lead_value.is_none() {
                        return Ok(AccessSolution::Solved(
                            SolvedPrecheckPrimitiveExpression::Common(Some(Val::Bool(
                                ignore_missing_value,
                            ))),
                        ));
                    }

                    let relational_predicate = compute_relational_predicate(
                        solver,
                        head,
                        lead_path,
                        pk_fields,
                        input_value,
                        lead_default,
                        request_context,
                        &solver.database,
                    )
                    .await?;

                    let f_expr = compute_function_expr(
                        &AccessPrimitiveExpressionPath {
                            column_path: tail.unwrap(),
                            field_path: lead.field_path.clone(),
                        },
                        parameter_name.clone(),
                        expr,
                    )?;

                    let new_input_value = input_value.map(|ctx| AccessInput {
                        value: ctx.value,
                        ignore_missing_value: ctx.ignore_missing_value,
                        aliases: {
                            if matches!(lead.field_path, FieldPath::Pk { .. }) {
                                ctx.aliases.clone()
                            } else {
                                let mut aliases = ctx.aliases.clone();
                                aliases.insert(
                                    parameter_name.as_str(),
                                    AccessInputPath(
                                        lead_path
                                            .iter()
                                            .map(|s| AccessInputPathElement::Property(s.as_str()))
                                            .collect(),
                                    ),
                                );
                                aliases
                            }
                        },
                    });

                    let solved_expr = solver
                        .solve(request_context, new_input_value.as_ref(), &f_expr)
                        .await?;

                    return Ok(solved_expr.map(|solved_expr| {
                        SolvedPrecheckPrimitiveExpression::Predicate(AbstractPredicate::and(
                            solved_expr.0,
                            relational_predicate,
                        ))
                    }));
                }
            };

            let function_input_value: Option<Result<Option<&Val>, _>> =
                input_value.as_ref().map(|ctx| {
                    ctx.resolve(AccessInputPath(
                        field_path
                            .iter()
                            .map(|s| AccessInputPathElement::Property(s.as_str()))
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
                                .map(|s| AccessInputPathElement::Property(s.as_str()))
                                .collect();
                            item_input_path_elements.push(AccessInputPathElement::Index(index));
                            AccessInputPath(item_input_path_elements)
                        };

                        let new_input_value = input_value.map(|ctx| AccessInput {
                            value: ctx.value,
                            ignore_missing_value: ctx.ignore_missing_value,
                            aliases: HashMap::from([(parameter_name.as_str(), item_input_path)]),
                        });

                        let solved_expr = solver
                            .solve(request_context, new_input_value.as_ref(), expr)
                            .await?;

                        if let AccessSolution::Solved(AbstractPredicateWrapper(p)) = solved_expr
                            && p == AbstractPredicate::True
                        {
                            result =
                                SolvedPrecheckPrimitiveExpression::Common(Some(Val::Bool(true)));
                            break;
                        }
                    }
                    Ok(AccessSolution::Solved(result))
                }
                _ => {
                    let ignore_missing_value = input_value
                        .as_ref()
                        .map(|ctx| ctx.ignore_missing_value)
                        .unwrap_or(true);

                    if ignore_missing_value {
                        Ok(AccessSolution::Solved(
                            SolvedPrecheckPrimitiveExpression::Common(Some(Val::Bool(true))),
                        ))
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

#[allow(clippy::too_many_arguments)]
async fn evaluate_relation(
    solver: &PostgresCoreSubsystem,
    request_context: &RequestContext<'_>,
    left: SolvedPrecheckPrimitiveExpression,
    right: SolvedPrecheckPrimitiveExpression,
    input_value: Option<&AccessInput<'_>>,
    database: &Database,
    ignore_missing_value: bool,
    column_predicate: ColumnPredicateFn,
    value_predicate: ValuePredicateFn,
) -> Result<AccessSolution<AbstractPredicateWrapper>, AccessSolverError> {
    match (left, right) {
        (SolvedPrecheckPrimitiveExpression::Common(None), _)
        | (_, SolvedPrecheckPrimitiveExpression::Common(None)) => Ok(AccessSolution::Unsolvable(
            AbstractPredicateWrapper(AbstractPredicate::False),
        )),

        (
            SolvedPrecheckPrimitiveExpression::Path(left_path, _),
            SolvedPrecheckPrimitiveExpression::Path(right_path, _),
        ) => {
            let (left_column_path, left_predicate) =
                resolve_path(solver, &left_path, input_value, request_context, database).await?;

            let (right_column_path, right_predicate) =
                resolve_path(solver, &right_path, input_value, request_context, database).await?;

            let core_predicate = match (left_column_path, right_column_path) {
                (Some(left_column_path), Some(right_column_path)) => {
                    column_predicate(left_column_path, right_column_path)
                }
                _ => ignore_missing_value.into(),
            };
            let relational_predicate = AbstractPredicate::and(left_predicate, right_predicate);

            Ok(AccessSolution::Solved(AbstractPredicateWrapper(
                AbstractPredicate::and(core_predicate, relational_predicate),
            )))
        }

        (
            SolvedPrecheckPrimitiveExpression::Common(Some(left_value)),
            SolvedPrecheckPrimitiveExpression::Common(Some(right_value)),
        ) => Ok(AccessSolution::Solved(
            value_predicate(&left_value, &right_value).into(),
        )),

        // The next two need to be handled separately, since we need to pass the left side
        // and right side to the predicate in the correct order. For example, `age > 18` is
        // different from `18 > age`.
        (
            SolvedPrecheckPrimitiveExpression::Common(Some(left_value)),
            SolvedPrecheckPrimitiveExpression::Path(right_path, parameter_name),
        ) => {
            process_path_common_expr(
                solver,
                right_path,
                parameter_name,
                left_value,
                input_value,
                request_context,
                database,
                |c1, c2| column_predicate(c2, c1),
                |v1, v2| value_predicate(v2, v1),
            )
            .await
        }

        (
            SolvedPrecheckPrimitiveExpression::Path(left_path, parameter_name),
            SolvedPrecheckPrimitiveExpression::Common(Some(right_value)),
        ) => {
            process_path_common_expr(
                solver,
                left_path,
                parameter_name,
                right_value,
                input_value,
                request_context,
                database,
                column_predicate,
                value_predicate,
            )
            .await
        }
        (
            SolvedPrecheckPrimitiveExpression::Predicate(left_predicate),
            SolvedPrecheckPrimitiveExpression::Common(Some(right_value)),
        ) => process_predicate_common_expr(left_predicate, right_value, false),
        (
            SolvedPrecheckPrimitiveExpression::Common(Some(left_value)),
            SolvedPrecheckPrimitiveExpression::Predicate(right_predicate),
        ) => process_predicate_common_expr(right_predicate, left_value, true),
        _ => Err(AccessSolverError::Generic("Unsupported expression".into())),
    }
}

async fn resolve_path(
    solver: &PostgresCoreSubsystem,
    path: &AccessPrimitiveExpressionPath,
    input_value: Option<&AccessInput<'_>>,
    request_context: &RequestContext<'_>,
    database: &Database,
) -> Result<(Option<ColumnPath>, AbstractPredicate), AccessSolverError> {
    let column_path = &path.column_path;
    let field_path = &path.field_path;

    match &field_path {
        FieldPath::Normal(field_path, default) => {
            let relational_predicate = AbstractPredicate::True;

            let value =
                resolve_value(solver, input_value, field_path, default, request_context).await?;

            let literal_column_path = compute_literal_column_path(
                value.as_ref().map(|v| v.as_ref()),
                column_path,
                database,
            )?;

            Ok((literal_column_path, relational_predicate))
        }
        FieldPath::Pk {
            lead,
            pk_fields,
            lead_default,
        } => {
            let (head, ..) = column_path.split_head();

            let relational_predicate = compute_relational_predicate(
                solver,
                head,
                lead,
                pk_fields,
                input_value,
                lead_default,
                request_context,
                database,
            )
            .await?;

            Ok((
                Some(ColumnPath::Physical(column_path.clone())),
                relational_predicate,
            ))
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn process_path_common_expr(
    solver: &PostgresCoreSubsystem,
    left_path: AccessPrimitiveExpressionPath,
    parameter_name: Option<String>,
    right_value: Val,
    input_value: Option<&AccessInput<'_>>,
    request_context: &RequestContext<'_>,
    database: &Database,
    column_predicate: impl Fn(ColumnPath, ColumnPath) -> AbstractPredicate,
    value_predicate: impl Fn(&Val, &Val) -> bool,
) -> Result<AccessSolution<AbstractPredicateWrapper>, AccessSolverError> {
    let ignore_missing_value = input_value
        .as_ref()
        .map(|ctx| ctx.ignore_missing_value)
        .unwrap_or(false);

    match &left_path.field_path {
        FieldPath::Normal(field_path, default) => {
            let left_value =
                resolve_value(solver, input_value, field_path, default, request_context).await?;

            match left_value {
                Some(left_value) => Ok(AccessSolution::Solved(
                    value_predicate(&left_value, &right_value).into(),
                )),
                None => {
                    if parameter_name
                        .as_ref()
                        .map(|p| p == &field_path[0])
                        .unwrap_or(false)
                    {
                        Ok(AccessSolution::Solved(AbstractPredicateWrapper(
                            column_predicate(
                                ColumnPath::Physical(left_path.column_path.clone()),
                                literal_column_path(
                                    &right_value,
                                    column_type(&left_path.column_path, database),
                                    false,
                                )
                                .unwrap(),
                            ),
                        )))
                    } else if ignore_missing_value {
                        Ok(AccessSolution::Unsolvable(AbstractPredicateWrapper(
                            AbstractPredicate::True,
                        )))
                    } else {
                        Ok(AccessSolution::Solved(AbstractPredicateWrapper(
                            column_predicate(
                                ColumnPath::Physical(left_path.column_path.clone()),
                                literal_column_path(
                                    &right_value,
                                    column_type(&left_path.column_path, database),
                                    false,
                                )
                                .unwrap(),
                            ),
                        )))
                    }
                }
            }
        }
        FieldPath::Pk {
            lead,
            pk_fields,
            lead_default,
        } => {
            let (left_head, left_tail_path) = left_path.column_path.split_head();

            let (left_column_path, right_column_path) =
                compute_relational_sides(&left_tail_path.unwrap(), &right_value, database)?;

            let core_predicate = column_predicate(left_column_path, right_column_path);
            let relational_predicate = compute_relational_predicate(
                solver,
                left_head,
                lead,
                pk_fields,
                input_value,
                lead_default,
                request_context,
                database,
            )
            .await?;
            Ok(AccessSolution::Solved(AbstractPredicateWrapper(
                AbstractPredicate::and(core_predicate, relational_predicate),
            )))
        }
    }
}

fn process_predicate_common_expr(
    predicate: AbstractPredicate,
    value: Val,
    commute: bool,
) -> Result<AccessSolution<AbstractPredicateWrapper>, AccessSolverError> {
    // Simplify the predicate if possible (i.e. one side is a boolean literal)
    match value {
        Val::Bool(value) => {
            let left_predicate = predicate.clone();
            let left_predicate = if value {
                left_predicate
            } else {
                use std::ops::Not;
                left_predicate.not()
            };

            Ok(AccessSolution::Solved(AbstractPredicateWrapper(
                left_predicate.clone(),
            )))
        }
        _ => {
            let predicate_column_path = ColumnPath::Predicate(Box::new(predicate.clone()));
            let boolean_type = BooleanColumnType;
            let value_column_path = literal_column_path(&value, &boolean_type, false).unwrap();

            Ok(AccessSolution::Solved(AbstractPredicateWrapper(
                if commute {
                    AbstractPredicate::eq(predicate_column_path, value_column_path)
                } else {
                    AbstractPredicate::eq(value_column_path, predicate_column_path)
                },
            )))
        }
    }
}

fn compute_relational_sides(
    tail_path: &PhysicalColumnPath,
    value: &Val,
    database: &Database,
) -> Result<(ColumnPath, ColumnPath), AccessSolverError> {
    let path_column_path = to_column_path(tail_path);

    let value_column_path =
        cast::literal_column_path(value, column_type(tail_path, database), false).map_err(|e| {
            AccessSolverError::Generic(format!("Failed to cast literal: '{value:?}': {e}").into())
        })?;

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
        .map_err(|e| {
            AccessSolverError::Generic(format!("Failed to cast literal: '{value:?}': {e}").into())
        })
}

#[allow(clippy::too_many_arguments)]
async fn compute_relational_predicate(
    solver: &PostgresCoreSubsystem,
    head_link: ColumnPathLink,
    lead: &[String],
    pk_fields: &[String],
    input_value: Option<&AccessInput<'_>>,
    default: &Option<PostgresFieldDefaultValue>,
    request_context: &RequestContext<'_>,
    database: &Database,
) -> Result<AbstractPredicate, AccessSolverError> {
    let lead_value = resolve_value(solver, input_value, lead, default, request_context).await?;

    let lead_value = lead_value.as_ref().map(|v| v.as_ref());

    use futures::stream::{self, TryStreamExt};

    match head_link {
        ColumnPathLink::Relation(relation) => {
            stream::iter(
                relation
                    .column_pairs
                    .iter()
                    .zip(pk_fields)
                    .map(Ok::<_, AccessSolverError>),
            )
            .try_fold(
                AbstractPredicate::True,
                |acc, (pair, pk_field)| async move {
                    let pk_field_path = vec![pk_field.clone()];

                    let pk_value = match lead_value {
                        Some(lead_value) => {
                            resolve_value(
                                solver,
                                Some(&AccessInput {
                                    value: lead_value,
                                    ignore_missing_value: false,
                                    aliases: input_value
                                        .map(|ctx| ctx.aliases.clone())
                                        .unwrap_or_default(),
                                }),
                                &pk_field_path,
                                default,
                                request_context,
                            )
                            .await
                        }
                        None => Ok(None),
                    }?;

                    match pk_value {
                        Some(pk_value) => {
                            let foreign_physical_column_path =
                                PhysicalColumnPath::leaf(pair.foreign_column_id);
                            let foreign_column_path =
                                ColumnPath::Physical(foreign_physical_column_path.clone());
                            let literal_column_path = compute_literal_column_path(
                                Some(pk_value.as_ref()),
                                &foreign_physical_column_path,
                                database,
                            )?;

                            let literal_column_path =
                                literal_column_path.unwrap_or(ColumnPath::Null);

                            Ok(AbstractPredicate::and(
                                acc,
                                AbstractPredicate::eq(foreign_column_path, literal_column_path),
                            ))
                        }
                        None => {
                            // If the pk value is not found, it means we are performing a nested mutation (where the value would be from the parent mutation)
                            Ok(AbstractPredicate::True)
                        }
                    }
                },
            )
            .await
        }
        ColumnPathLink::Leaf(column_id) => Err(AccessSolverError::Generic(
            format!("Invalid column path: {:?}", column_id).into(),
        )),
    }
}

fn column_type<'a>(
    physical_column_path: &PhysicalColumnPath,
    database: &'a Database,
) -> &'a dyn PhysicalColumnType {
    physical_column_path
        .leaf_column()
        .get_column(database)
        .typ
        .inner()
}

async fn resolve_value<'a>(
    solver: &PostgresCoreSubsystem,
    input_value: Option<&AccessInput<'a>>,
    path: &'a [String],
    default: &'a Option<PostgresFieldDefaultValue>,
    request_context: &'a RequestContext<'a>,
) -> Result<Option<MaybeOwned<'a, Val>>, AccessSolverError> {
    let value: Option<Result<Option<&Val>, _>> = input_value.as_ref().map(|ctx| {
        ctx.resolve(AccessInputPath(
            path.iter()
                .map(|s| AccessInputPathElement::Property(s.as_str()))
                .collect(),
        ))
    });

    let value = value.transpose()?.flatten();

    use core_resolver::context_extractor::ContextExtractor;
    match (value, default) {
        (Some(value), _) => Ok(Some(MaybeOwned::Borrowed(value))),
        (None, Some(default)) => match default {
            PostgresFieldDefaultValue::Static(value) => Ok(Some(MaybeOwned::Borrowed(value))),
            PostgresFieldDefaultValue::Dynamic(context_selection) => {
                let value = solver
                    .extract_context_selection(request_context, context_selection)
                    .await?
                    .cloned();
                Ok(value.map(|v| v.into()))
            }
            PostgresFieldDefaultValue::Function(_) => Ok(None),
            PostgresFieldDefaultValue::AutoIncrement(_) => Ok(None),
        },
        _ => Ok(None),
    }
}

fn compute_function_expr(
    lead_path: &AccessPrimitiveExpressionPath,
    function_param_name: String,
    function_expr: &AccessPredicateExpression<PrecheckAccessPrimitiveExpression>,
) -> Result<AccessPredicateExpression<PrecheckAccessPrimitiveExpression>, AccessSolverError> {
    fn function_elem_path(
        lead_path: &AccessPrimitiveExpressionPath,
        function_param_name: String,
        expr: PrecheckAccessPrimitiveExpression,
    ) -> Result<PrecheckAccessPrimitiveExpression, AccessSolverError> {
        match expr {
            PrecheckAccessPrimitiveExpression::Path(function_path, parameter_name) => {
                // We may have expression like `self.documentUser.some(du => du.read)`, in which case we want to join the column path
                // to form `self.documentUser.read`.
                //
                // However, if the lead path is `self.documentUser.some(du => du.id === self.id)`, we don't want to join the column path
                // for the `self.id` part.
                Ok(PrecheckAccessPrimitiveExpression::Path(
                    match &parameter_name {
                        Some(parameter_name) if parameter_name == &function_param_name => lead_path
                            .clone()
                            .with_function_context(function_path, parameter_name.clone())?,
                        _ => function_path,
                    },
                    parameter_name.clone(),
                ))
            }
            PrecheckAccessPrimitiveExpression::Function(_, _) => Err(AccessSolverError::Generic(
                "Cannot have a function call inside another function call".into(),
            )),
            expr => Ok(expr),
        }
    }

    match function_expr {
        AccessPredicateExpression::LogicalOp(op) => match op {
            AccessLogicalExpression::Not(p) => {
                let updated_expr = compute_function_expr(lead_path, function_param_name, p)?;
                Ok(AccessPredicateExpression::LogicalOp(
                    AccessLogicalExpression::Not(Box::new(updated_expr)),
                ))
            }
            AccessLogicalExpression::And(left, right) => {
                let updated_left =
                    compute_function_expr(lead_path, function_param_name.clone(), left)?;
                let updated_right = compute_function_expr(lead_path, function_param_name, right)?;

                Ok(AccessPredicateExpression::LogicalOp(
                    AccessLogicalExpression::And(Box::new(updated_left), Box::new(updated_right)),
                ))
            }
            AccessLogicalExpression::Or(left, right) => {
                let updated_left =
                    compute_function_expr(lead_path, function_param_name.clone(), left)?;
                let updated_right = compute_function_expr(lead_path, function_param_name, right)?;

                Ok(AccessPredicateExpression::LogicalOp(
                    AccessLogicalExpression::Or(Box::new(updated_left), Box::new(updated_right)),
                ))
            }
        },
        AccessPredicateExpression::RelationalOp(op) => {
            let combiner = op.combiner();
            let (left, right) = op.sides();

            let updated_left =
                function_elem_path(lead_path, function_param_name.clone(), left.clone())?;
            let updated_right = function_elem_path(lead_path, function_param_name, right.clone())?;
            Ok(AccessPredicateExpression::RelationalOp(combiner(
                Box::new(updated_left),
                Box::new(updated_right),
            )))
        }
        AccessPredicateExpression::BooleanLiteral(value) => {
            Ok(AccessPredicateExpression::BooleanLiteral(*value))
        }
    }
}
