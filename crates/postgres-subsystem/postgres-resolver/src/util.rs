// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use futures::stream::TryStreamExt;
use indexmap::IndexMap;
use postgres_model::access::{
    DatabaseAccessPrimitiveExpression, InputAccessPrimitiveExpression, UpdateAccessExpression,
};
use postgres_model::types::EntityType;

use crate::{postgres_execution_error::PostgresExecutionError, sql_mapper::SQLOperationKind};
use core_plugin_interface::core_model::{
    access::AccessPredicateExpression, types::OperationReturnType,
};
use core_plugin_interface::core_resolver::{
    access_solver::AccessSolver, context::RequestContext, validation::field::ValidatedField,
    value::Val,
};
use exo_sql::{AbstractPredicate, Predicate, TableId};
use postgres_model::{
    query::{CollectionQuery, PkQuery},
    subsystem::PostgresSubsystem,
};

pub type Arguments = IndexMap<String, Val>;

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
                    return Err(PostgresExecutionError::Authorization);
                } else {
                    access_predicate
                }
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
                    futures::stream::iter(selection.iter().map(Ok))
                        .try_fold(entity_access, |access_predicate, selection_field| async {
                            let postgres_field = return_type.field_by_name(&selection_field.name);

                            let field_access_predicate = match postgres_field {
                                Some(postgres_field) => {
                                    crate::util::check_retrieve_access(
                                        &subsystem.database_access_expressions
                                            [postgres_field.access.read],
                                        subsystem,
                                        request_context,
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
                        })
                        .await
                }?
            }
            SQLOperationKind::Update => {
                check_update_access(
                    &return_type.access.update,
                    subsystem,
                    request_context,
                    input_context,
                )
                .await?
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

pub fn find_arg<'a>(arguments: &'a Arguments, arg_name: &str) -> Option<&'a Val> {
    arguments.iter().find_map(|argument| {
        let (argument_name, argument_value) = argument;
        if arg_name == argument_name {
            Some(argument_value)
        } else {
            None
        }
    })
}

pub(crate) fn get_argument_field<'a>(argument_value: &'a Val, field_name: &str) -> Option<&'a Val> {
    match argument_value {
        Val::Object(value) => value.get(field_name),
        _ => None,
    }
}

///
/// # Returns
/// - A (table associated with the return type, pk query, collection query) tuple.
pub(crate) fn return_type_info<'a>(
    return_type: &'a OperationReturnType<EntityType>,
    subsystem: &'a PostgresSubsystem,
) -> (TableId, &'a PkQuery, &'a CollectionQuery) {
    let typ = return_type.typ(&subsystem.entity_types);

    (
        typ.table_id,
        &subsystem.pk_queries[typ.pk_query],
        &subsystem.collection_queries[typ.collection_query],
    )
}
