// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use core_model::mapped_arena::SerializableSlabIndex;
use core_resolver::access_solver::AccessInput;
use futures::stream::TryStreamExt;
use postgres_core_model::access::{
    CreationAccessExpression, DatabaseAccessPrimitiveExpression, PrecheckAccessPrimitiveExpression,
    UpdateAccessExpression,
};
use postgres_core_model::types::{EntityType, PostgresField};

use postgres_core_resolver::postgres_execution_error::PostgresExecutionError;

use super::sql_mapper::SQLOperationKind;

use common::context::RequestContext;
use common::value::Val;
use core_model::access::AccessPredicateExpression;
use core_resolver::{access_solver::AccessSolver, validation::field::ValidatedField};
use exo_sql::{AbstractPredicate, Predicate};
use postgres_graphql_model::subsystem::PostgresGraphQLSubsystem;

pub(crate) struct AccessCheckOutcome {
    pub precheck_predicate: AbstractPredicate,
    pub entity_predicate: AbstractPredicate,
    pub unauthorized_fields: Vec<String>,
}

struct SelectionAccessOutcome {
    predicate: AbstractPredicate,
    unauthorized_fields: Vec<String>,
}

pub(crate) async fn check_access<'a>(
    return_type: &'a EntityType,
    selection: &'a [ValidatedField],
    kind: &SQLOperationKind,
    subsystem: &'a PostgresGraphQLSubsystem,
    request_context: &'a RequestContext<'a>,
    input_value: Option<&AccessInput<'a>>,
) -> Result<AccessCheckOutcome, PostgresExecutionError> {
    let (precheck_predicate, entity_predicate, unauthorized_fields) = {
        match kind {
            SQLOperationKind::Create => {
                let entity_access = check_create_access(
                    &return_type.access.creation,
                    subsystem,
                    request_context,
                    input_value,
                )
                .await?;

                if entity_access == Predicate::False {
                    Err(PostgresExecutionError::Authorization)
                } else {
                    let field_access_predicate = check_input_access(
                        input_value,
                        return_type,
                        subsystem,
                        request_context,
                        |field| field.access.creation.precheck,
                    )
                    .await?;

                    if field_access_predicate == AbstractPredicate::False {
                        Err(PostgresExecutionError::Authorization)
                    } else {
                        let precheck_predicate =
                            AbstractPredicate::and(entity_access, field_access_predicate);
                        Ok((precheck_predicate, AbstractPredicate::True, vec![]))
                    }
                }?
            }
            SQLOperationKind::Retrieve => {
                let entity_access = check_retrieve_access(
                    &subsystem.core_subsystem.database_access_expressions[return_type.access.read],
                    subsystem,
                    request_context,
                )
                .await?;

                if entity_access == Predicate::False {
                    // Short circuit this common case
                    Err(PostgresExecutionError::Authorization)
                } else {
                    let SelectionAccessOutcome {
                        predicate: field_access_predicate,
                        unauthorized_fields,
                    } = check_selection_access(selection, return_type, subsystem, request_context)
                        .await?;

                    Ok((
                        AbstractPredicate::True,
                        AbstractPredicate::and(entity_access, field_access_predicate),
                        unauthorized_fields,
                    ))
                }?
            }
            SQLOperationKind::Update => {
                let (precheck_predicate, entity_predicate) = check_update_access(
                    &return_type.access.update,
                    subsystem,
                    request_context,
                    input_value,
                )
                .await?;

                if precheck_predicate == AbstractPredicate::False
                    || entity_predicate == AbstractPredicate::False
                {
                    // Short circuit this common case
                    Err(PostgresExecutionError::Authorization)
                } else {
                    let field_access_predicate = check_input_access(
                        input_value,
                        return_type,
                        subsystem,
                        request_context,
                        |field| field.access.update.precheck,
                    )
                    .await?;

                    if field_access_predicate == AbstractPredicate::False {
                        Err(PostgresExecutionError::Authorization)
                    } else {
                        let database_predicate =
                            AbstractPredicate::and(entity_predicate, field_access_predicate);
                        Ok((precheck_predicate, database_predicate, vec![]))
                    }
                }?
            }
            SQLOperationKind::Delete => {
                let (precheck_predicate, entity_predicate) = check_delete_access(
                    &subsystem.core_subsystem.database_access_expressions
                        [return_type.access.delete],
                    subsystem,
                    request_context,
                )
                .await?;

                (precheck_predicate, entity_predicate, vec![])
            }
        }
    };

    if precheck_predicate == AbstractPredicate::False || entity_predicate == Predicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        Err(PostgresExecutionError::Authorization)
    } else {
        Ok(AccessCheckOutcome {
            precheck_predicate,
            entity_predicate,
            unauthorized_fields,
        })
    }
}

async fn check_create_access<'a>(
    expr: &CreationAccessExpression,
    subsystem: &'a PostgresGraphQLSubsystem,
    request_context: &'a RequestContext<'a>,
    input_value: Option<&AccessInput<'a>>,
) -> Result<AbstractPredicate, PostgresExecutionError> {
    let precheck_predicate = subsystem
        .core_subsystem
        .solve(
            request_context,
            input_value,
            &subsystem.core_subsystem.precheck_expressions[expr.precheck],
        )
        .await?
        .map(|predicate| predicate.0)
        .resolve();

    if precheck_predicate == AbstractPredicate::False {
        Err(PostgresExecutionError::Authorization)
    } else {
        Ok(precheck_predicate)
    }
}

pub(super) async fn check_retrieve_access<'a>(
    expr: &AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
    subsystem: &'a PostgresGraphQLSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<AbstractPredicate, PostgresExecutionError> {
    Ok(subsystem
        .core_subsystem
        .solve(request_context, None, expr)
        .await?
        .map(|p| p.0)
        .resolve())
}

async fn check_update_access<'a>(
    expr: &UpdateAccessExpression,
    subsystem: &'a PostgresGraphQLSubsystem,
    request_context: &'a RequestContext<'a>,
    input_value: Option<&AccessInput<'a>>,
) -> Result<(AbstractPredicate, AbstractPredicate), PostgresExecutionError> {
    // First check the input predicate (i.e. the "data" parameter matches the access predicate)
    let precheck_predicate = subsystem
        .core_subsystem
        .solve(
            request_context,
            input_value,
            &subsystem.core_subsystem.precheck_expressions[expr.precheck],
        )
        .await?
        .map(|predicate| predicate.0)
        .resolve();

    // Input predicate cannot have a residue (i.e. it must fully evaluated to true or false)
    if precheck_predicate == AbstractPredicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        return Err(PostgresExecutionError::Authorization);
    }

    // Now compute the database access predicate (the "where" clause to the update statement)
    let database_predicate = subsystem
        .core_subsystem
        .solve(
            request_context,
            None,
            &subsystem.core_subsystem.database_access_expressions[expr.database],
        )
        .await?
        .map(|p| p.0)
        .resolve();

    Ok((precheck_predicate, database_predicate))
}

async fn check_delete_access<'a>(
    expr: &AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
    subsystem: &'a PostgresGraphQLSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<(AbstractPredicate, AbstractPredicate), PostgresExecutionError> {
    Ok((
        AbstractPredicate::True,
        subsystem
            .core_subsystem
            .solve(request_context, None, expr)
            .await?
            .map(|p| p.0)
            .resolve(),
    ))
}

async fn check_selection_access<'a>(
    selection: &'a [ValidatedField],
    return_type: &'a EntityType,
    subsystem: &'a PostgresGraphQLSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<SelectionAccessOutcome, PostgresExecutionError> {
    futures::stream::iter(selection.iter().map(Ok))
        .try_fold(
            SelectionAccessOutcome {
                predicate: AbstractPredicate::True,
                unauthorized_fields: vec![],
            },
            |mut acc, selection_field| async {
                let postgres_field = return_type.field_by_name(&selection_field.name);

                let field_access_predicate = match postgres_field {
                    Some(postgres_field) => {
                        check_retrieve_access(
                            &subsystem.core_subsystem.database_access_expressions
                                [postgres_field.access.read],
                            subsystem,
                            request_context,
                        )
                        .await
                    }
                    None => {
                        match return_type.vector_distance_field_by_name(&selection_field.name) {
                            Some(vector_distance_field) => {
                                check_retrieve_access(
                                    &subsystem.core_subsystem.database_access_expressions
                                        [vector_distance_field.access.read],
                                    subsystem,
                                    request_context,
                                )
                                .await
                            }
                            None => Ok(AbstractPredicate::True),
                        }
                    }
                }?;

                if field_access_predicate == AbstractPredicate::False {
                    acc.unauthorized_fields.push(selection_field.output_name());
                    Ok(acc)
                } else {
                    acc.predicate = AbstractPredicate::and(acc.predicate, field_access_predicate);
                    Ok(acc)
                }
            },
        )
        .await
}

async fn check_input_access<'a>(
    input_value: Option<&AccessInput<'a>>,
    return_type: &'a EntityType,
    subsystem: &'a PostgresGraphQLSubsystem,
    request_context: &'a RequestContext<'a>,
    field_access: fn(
        &PostgresField<EntityType>,
    ) -> SerializableSlabIndex<
        AccessPredicateExpression<PrecheckAccessPrimitiveExpression>,
    >,
) -> Result<AbstractPredicate, PostgresExecutionError> {
    match input_value.as_ref().map(|ctx| ctx.value) {
        None => Ok(AbstractPredicate::True),
        Some(Val::Object(elems)) => {
            futures::stream::iter(elems.iter().map(Ok))
                .try_fold(
                    AbstractPredicate::True,
                    |access_predicate, (elem_name, _)| async {
                        let postgres_field = return_type.field_by_name(elem_name);

                        let field_access_predicate = match postgres_field {
                            Some(postgres_field) => {
                                let access_input = AccessInput {
                                    value: &Val::Object(elems.clone()),
                                    ignore_missing_value: false,
                                    aliases: HashMap::new(),
                                };

                                let input_predicate = subsystem
                                    .core_subsystem
                                    .solve(
                                        request_context,
                                        Some(&access_input),
                                        &subsystem.core_subsystem.precheck_expressions
                                            [field_access(postgres_field)],
                                    )
                                    .await?
                                    .map(|predicate| predicate.0)
                                    .resolve();

                                if input_predicate == AbstractPredicate::False {
                                    Err(PostgresExecutionError::Authorization)
                                } else {
                                    Ok(input_predicate)
                                }
                            }
                            None => Ok(AbstractPredicate::True),
                        }?;

                        if field_access_predicate == AbstractPredicate::False {
                            Err(PostgresExecutionError::Authorization)
                        } else {
                            Ok(AbstractPredicate::and(
                                access_predicate,
                                field_access_predicate,
                            ))
                        }
                    },
                )
                .await
        }
        _ => Ok(AbstractPredicate::True),
    }
}
