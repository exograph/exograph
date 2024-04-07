// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use core_plugin_interface::core_model::mapped_arena::SerializableSlabIndex;
use futures::stream::TryStreamExt;
use postgres_model::access::{
    DatabaseAccessPrimitiveExpression, InputAccessPrimitiveExpression, UpdateAccessExpression,
};
use postgres_model::types::{EntityType, PostgresField};

use crate::{postgres_execution_error::PostgresExecutionError, sql_mapper::SQLOperationKind};
use core_plugin_interface::core_model::access::AccessPredicateExpression;
use core_plugin_interface::core_resolver::{
    access_solver::AccessSolver, context::RequestContext, validation::field::ValidatedField,
    value::Val,
};
use exo_sql::{AbstractPredicate, Predicate};
use postgres_model::subsystem::PostgresSubsystem;

pub(crate) async fn check_access<'a>(
    return_type: &'a EntityType,
    selection: &'a [ValidatedField],
    kind: &SQLOperationKind,
    subsystem: &'a PostgresSubsystem,
    request_context: &'a RequestContext<'a>,
    input_context: Option<&'a Val>,
) -> Result<AbstractPredicate, PostgresExecutionError> {
    let access_predicate = {
        match kind {
            SQLOperationKind::Create => {
                let access_predicate = check_create_access(
                    &subsystem.input_access_expressions[return_type.access.creation],
                    subsystem,
                    request_context,
                    input_context,
                )
                .await?;

                // For creation, the access predicate must be `True` (i.e. it must not have any residual
                // conditions) The `False` case is already handled by the check_access function (by rejecting
                // the request)
                if access_predicate != Predicate::True {
                    Err(PostgresExecutionError::Authorization)
                } else {
                    let field_access_predicate = check_input_access(
                        input_context,
                        return_type,
                        subsystem,
                        request_context,
                        |field| field.access.creation,
                    )
                    .await?;

                    if field_access_predicate != AbstractPredicate::True {
                        Err(PostgresExecutionError::Authorization)
                    } else {
                        Ok(AbstractPredicate::True)
                    }
                }?
            }
            SQLOperationKind::Retrieve => {
                let entity_access = check_retrieve_access(
                    &subsystem.database_access_expressions[return_type.access.read],
                    subsystem,
                    request_context,
                )
                .await?;

                if entity_access == Predicate::False {
                    // Short circuit this common case
                    Err(PostgresExecutionError::Authorization)
                } else {
                    let field_access_predicate =
                        check_selection_access(selection, return_type, subsystem, request_context)
                            .await?;
                    if field_access_predicate == AbstractPredicate::False {
                        Err(PostgresExecutionError::Authorization)
                    } else {
                        Ok(AbstractPredicate::and(
                            entity_access,
                            field_access_predicate,
                        ))
                    }
                }?
            }
            SQLOperationKind::Update => {
                let entity_access = check_update_access(
                    &return_type.access.update,
                    subsystem,
                    request_context,
                    input_context,
                )
                .await?;

                if entity_access == Predicate::False {
                    // Short circuit this common case
                    Err(PostgresExecutionError::Authorization)
                } else {
                    let field_access_predicate = check_input_access(
                        input_context,
                        return_type,
                        subsystem,
                        request_context,
                        |field| field.access.update.input,
                    )
                    .await?;
                    if field_access_predicate == AbstractPredicate::False {
                        Err(PostgresExecutionError::Authorization)
                    } else {
                        Ok(AbstractPredicate::and(
                            entity_access,
                            field_access_predicate,
                        ))
                    }
                }?
            }
            SQLOperationKind::Delete => {
                check_delete_access(
                    &subsystem.database_access_expressions[return_type.access.delete],
                    subsystem,
                    request_context,
                )
                .await?
            }
        }
    };

    if access_predicate == AbstractPredicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        Err(PostgresExecutionError::Authorization)
    } else {
        Ok(access_predicate)
    }
}

async fn check_create_access<'a>(
    expr: &AccessPredicateExpression<InputAccessPrimitiveExpression>,
    subsystem: &'a PostgresSubsystem,
    request_context: &'a RequestContext<'a>,
    input_context: Option<&'a Val>,
) -> Result<AbstractPredicate, PostgresExecutionError> {
    Ok(subsystem
        .solve(request_context, input_context, expr)
        .await?
        .map(|predicate| predicate.0)
        .unwrap_or(AbstractPredicate::False))
}

pub(super) async fn check_retrieve_access<'a>(
    expr: &AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
    subsystem: &'a PostgresSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<AbstractPredicate, PostgresExecutionError> {
    Ok(subsystem
        .solve(request_context, None, expr)
        .await?
        .map(|p| p.0)
        .unwrap_or(AbstractPredicate::False))
}

async fn check_update_access<'a>(
    expr: &UpdateAccessExpression,
    subsystem: &'a PostgresSubsystem,
    request_context: &'a RequestContext<'a>,
    input_context: Option<&'a Val>,
) -> Result<AbstractPredicate, PostgresExecutionError> {
    // First check the input predicate (i.e. the "data" parameter matches the access predicate)
    let input_predicate = subsystem
        .solve(
            request_context,
            input_context,
            &subsystem.input_access_expressions[expr.input],
        )
        .await?
        .map(|p| p.0)
        .unwrap_or(AbstractPredicate::False);

    // Input predicate cannot have a residue (i.e. it must fully evaluated to true or false)
    if input_predicate != AbstractPredicate::True {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        return Err(PostgresExecutionError::Authorization);
    }

    // Now compute the database access predicate (the "where" clause to the update statement)
    Ok(subsystem
        .solve(
            request_context,
            None,
            &subsystem.database_access_expressions[expr.database],
        )
        .await?
        .map(|p| p.0)
        .unwrap_or(AbstractPredicate::False))
}

async fn check_delete_access<'a>(
    expr: &AccessPredicateExpression<DatabaseAccessPrimitiveExpression>,
    subsystem: &'a PostgresSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<AbstractPredicate, PostgresExecutionError> {
    Ok(subsystem
        .solve(request_context, None, expr)
        .await?
        .map(|p| p.0)
        .unwrap_or(AbstractPredicate::False))
}

async fn check_selection_access<'a>(
    selection: &'a [ValidatedField],
    return_type: &'a EntityType,
    subsystem: &'a PostgresSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<AbstractPredicate, PostgresExecutionError> {
    futures::stream::iter(selection.iter().map(Ok))
        .try_fold(
            AbstractPredicate::True,
            |access_predicate, selection_field| async {
                let postgres_field = return_type.field_by_name(&selection_field.name);

                let field_access_predicate = match postgres_field {
                    Some(postgres_field) => {
                        check_retrieve_access(
                            &subsystem.database_access_expressions[postgres_field.access.read],
                            subsystem,
                            request_context,
                        )
                        .await
                    }
                    None => {
                        match return_type.vector_distance_field_by_name(&selection_field.name) {
                            Some(vector_distance_field) => {
                                check_retrieve_access(
                                    &subsystem.database_access_expressions
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

async fn check_input_access<'a>(
    input_context: Option<&'a Val>,
    return_type: &'a EntityType,
    subsystem: &'a PostgresSubsystem,
    request_context: &'a RequestContext<'a>,
    field_access: fn(
        &PostgresField<EntityType>,
    ) -> SerializableSlabIndex<
        AccessPredicateExpression<InputAccessPrimitiveExpression>,
    >,
) -> Result<AbstractPredicate, PostgresExecutionError> {
    match input_context {
        None => Ok(AbstractPredicate::True),
        Some(Val::Object(elems)) => {
            futures::stream::iter(elems.iter().map(Ok))
                .try_fold(
                    AbstractPredicate::True,
                    |access_predicate, (elem_name, elem_value)| async {
                        let postgres_field = return_type.field_by_name(elem_name);

                        let field_access_predicate = match postgres_field {
                            Some(postgres_field) => {
                                check_create_access(
                                    &subsystem.input_access_expressions
                                        [field_access(postgres_field)],
                                    subsystem,
                                    request_context,
                                    Some(elem_value),
                                )
                                .await
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
