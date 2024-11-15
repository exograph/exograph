// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::{
    auth_util::check_access,
    postgres_execution_error::PostgresExecutionError,
    sql_mapper::SQLOperationKind,
    util::{find_arg, return_type_info},
};
use crate::{
    create_data_param_mapper::InsertOperation, operation_resolver::OperationResolver,
    postgres_query::compute_select, predicate_mapper::compute_predicate, sql_mapper::SQLMapper,
    update_data_param_mapper::UpdateOperation,
};
use async_trait::async_trait;
use common::context::RequestContext;
use core_plugin_interface::core_model::types::OperationReturnType;
use core_plugin_interface::core_resolver::validation::field::ValidatedField;
use exo_sql::{
    AbstractDelete, AbstractInsert, AbstractOperation, AbstractPredicate, AbstractSelect,
    AbstractUpdate, Predicate,
};
use postgres_model::{
    mutation::{DataParameter, PostgresMutation, PostgresMutationParameters},
    predicate::PredicateParameter,
    subsystem::PostgresSubsystem,
    types::EntityType,
};

#[async_trait]
impl OperationResolver for PostgresMutation {
    async fn resolve<'a>(
        &'a self,
        field: &'a ValidatedField,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresSubsystem,
    ) -> Result<AbstractOperation, PostgresExecutionError> {
        let return_type = &self.return_type;

        // Compute a select without any **user-specified** predicate, order-by etc. The surrounding
        // mutation will add an appropriate predicate (for example, an update mutation will add a
        // predicate to restrict the select to only ids that had been updated). We do, however, add
        // access-control predicates in `compute_select`.
        let abstract_select = compute_select(
            AbstractPredicate::True,
            None,
            None,
            None,
            return_type,
            &field.subfields,
            subsystem,
            request_context,
        )
        .await?;

        Ok(match &self.parameters {
            PostgresMutationParameters::Create(data_param) => AbstractOperation::Insert(
                create_operation(
                    data_param,
                    field,
                    abstract_select,
                    subsystem,
                    request_context,
                )
                .await?,
            ),
            PostgresMutationParameters::Delete(predicate_param) => AbstractOperation::Delete(
                delete_operation(
                    return_type,
                    predicate_param,
                    field,
                    abstract_select,
                    subsystem,
                    request_context,
                )
                .await?,
            ),
            PostgresMutationParameters::Update {
                data_param,
                predicate_param,
            } => AbstractOperation::Update(
                update_operation(
                    return_type,
                    data_param,
                    predicate_param,
                    field,
                    abstract_select,
                    subsystem,
                    request_context,
                )
                .await?,
            ),
        })
    }
}

async fn create_operation<'content>(
    data_param: &'content DataParameter,
    field: &'content ValidatedField,
    select: AbstractSelect,
    subsystem: &'content PostgresSubsystem,
    request_context: &'content RequestContext<'content>,
) -> Result<AbstractInsert, PostgresExecutionError> {
    let data_arg = find_arg(&field.arguments, &data_param.name);

    match data_arg {
        Some(argument) => {
            InsertOperation { data_param, select }
                .to_sql(argument, subsystem, request_context)
                .await
        }
        None => Err(PostgresExecutionError::MissingArgument(
            data_param.name.clone(),
        )),
    }
}

async fn delete_operation<'content>(
    return_type: &'content OperationReturnType<EntityType>,
    predicate_param: &'content PredicateParameter,
    field: &'content ValidatedField,
    select: AbstractSelect,
    subsystem: &'content PostgresSubsystem,
    request_context: &'content RequestContext<'content>,
) -> Result<AbstractDelete, PostgresExecutionError> {
    let (table_id, _, _) = return_type_info(return_type, subsystem);

    let access_predicate = check_access(
        return_type.typ(&subsystem.entity_types),
        &field.subfields,
        &SQLOperationKind::Delete,
        subsystem,
        request_context,
        None,
    )
    .await?;

    let arg_predicate = compute_predicate(
        predicate_param,
        &field.arguments,
        subsystem,
        request_context,
    )
    .await?;
    let predicate = Predicate::and(access_predicate, arg_predicate);

    Ok(AbstractDelete {
        table_id,
        predicate,
        selection: select,
    })
}

async fn update_operation<'content>(
    return_type: &'content OperationReturnType<EntityType>,
    data_param: &'content DataParameter,
    predicate_param: &'content PredicateParameter,
    field: &'content ValidatedField,
    select: AbstractSelect,
    subsystem: &'content PostgresSubsystem,
    request_context: &'content RequestContext<'content>,
) -> Result<AbstractUpdate, PostgresExecutionError> {
    let data_arg = find_arg(&field.arguments, &data_param.name);
    let access_predicate = check_access(
        return_type.typ(&subsystem.entity_types),
        &field.subfields,
        &SQLOperationKind::Update,
        subsystem,
        request_context,
        data_arg,
    )
    .await?;

    let arg_predicate = compute_predicate(
        predicate_param,
        &field.arguments,
        subsystem,
        request_context,
    )
    .await?;
    let predicate = Predicate::and(access_predicate, arg_predicate);

    match data_arg {
        Some(argument) => {
            UpdateOperation {
                data_param,
                predicate,
                select,
                return_type,
            }
            .to_sql(argument, subsystem, request_context)
            .await
        }
        None => Err(PostgresExecutionError::MissingArgument(
            data_param.name.clone(),
        )),
    }
}
