// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::{auth_util::check_access, sql_mapper::SQLOperationKind, util::find_arg};

use postgres_core_resolver::postgres_execution_error::PostgresExecutionError;

use crate::{
    create_data_param_mapper::InsertOperation,
    operation_resolver::{OperationResolution, OperationResolver},
    postgres_query::compute_select,
    predicate_mapper::compute_predicate,
    sql_mapper::SQLMapper,
    update_data_param_mapper::UpdateOperation,
};
use async_trait::async_trait;
use common::context::RequestContext;
use core_plugin_interface::core_resolver::validation::field::ValidatedField;
use core_plugin_interface::{
    core_model::types::OperationReturnType, core_resolver::access_solver::AccessInputContext,
};
use exo_sql::{
    AbstractDelete, AbstractInsert, AbstractOperation, AbstractPredicate, AbstractSelect,
    AbstractUpdate, Predicate,
};
use postgres_core_model::types::EntityType;
use postgres_graphql_model::{
    mutation::{DataParameter, PostgresMutation, PostgresMutationParameters},
    predicate::PredicateParameter,
    subsystem::PostgresGraphQLSubsystem,
};

#[async_trait]
impl OperationResolver for PostgresMutation {
    async fn resolve<'a>(
        &'a self,
        field: &'a ValidatedField,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresGraphQLSubsystem,
    ) -> Result<OperationResolution<AbstractOperation>, PostgresExecutionError> {
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
            PostgresMutationParameters::Create(data_param) => {
                let insert = create_operation(
                    data_param,
                    field,
                    abstract_select,
                    subsystem,
                    request_context,
                )
                .await?;

                OperationResolution {
                    precheck_predicates: insert.precheck_predicates,
                    operation: AbstractOperation::Insert(insert.operation),
                }
            }
            PostgresMutationParameters::Delete(predicate_params) => {
                let delete = delete_operation(
                    return_type,
                    predicate_params,
                    field,
                    abstract_select,
                    subsystem,
                    request_context,
                )
                .await?;

                OperationResolution {
                    precheck_predicates: delete.precheck_predicates,
                    operation: AbstractOperation::Delete(delete.operation),
                }
            }
            PostgresMutationParameters::Update {
                data_param,
                predicate_params,
            } => {
                let update = update_operation(
                    return_type,
                    data_param,
                    predicate_params,
                    field,
                    abstract_select,
                    subsystem,
                    request_context,
                )
                .await?;

                OperationResolution {
                    precheck_predicates: update.precheck_predicates,
                    operation: AbstractOperation::Update(update.operation),
                }
            }
        })
    }
}

async fn create_operation<'content>(
    data_param: &'content DataParameter,
    field: &'content ValidatedField,
    select: AbstractSelect,
    subsystem: &'content PostgresGraphQLSubsystem,
    request_context: &'content RequestContext<'content>,
) -> Result<OperationResolution<AbstractInsert>, PostgresExecutionError> {
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
    predicate_params: &'content [PredicateParameter],
    field: &'content ValidatedField,
    select: AbstractSelect,
    subsystem: &'content PostgresGraphQLSubsystem,
    request_context: &'content RequestContext<'content>,
) -> Result<OperationResolution<AbstractDelete>, PostgresExecutionError> {
    let table_id = subsystem.core_subsystem.entity_types[return_type.typ_id()].table_id;

    let (precheck_predicate, entity_predicate) = check_access(
        return_type.typ(&subsystem.core_subsystem.entity_types),
        &field.subfields,
        &SQLOperationKind::Delete,
        subsystem,
        request_context,
        None,
    )
    .await?;

    let arg_predicate = compute_predicate(
        &predicate_params.iter().collect::<Vec<_>>(),
        &field.arguments,
        subsystem,
        request_context,
    )
    .await?;
    let predicate = Predicate::and(entity_predicate, arg_predicate);

    Ok(OperationResolution {
        precheck_predicates: vec![precheck_predicate],
        operation: AbstractDelete {
            table_id,
            predicate,
            selection: select,
        },
    })
}

async fn update_operation<'content>(
    return_type: &'content OperationReturnType<EntityType>,
    data_param: &'content DataParameter,
    predicate_param: &'content [PredicateParameter],
    field: &'content ValidatedField,
    select: AbstractSelect,
    subsystem: &'content PostgresGraphQLSubsystem,
    request_context: &'content RequestContext<'content>,
) -> Result<OperationResolution<AbstractUpdate>, PostgresExecutionError> {
    let data_arg = find_arg(&field.arguments, &data_param.name);
    let input_context = data_arg.map(|arg| AccessInputContext {
        value: arg,
        ignore_missing_context: true,
    });
    let (precheck_predicate, entity_predicate) = check_access(
        return_type.typ(&subsystem.core_subsystem.entity_types),
        &field.subfields,
        &SQLOperationKind::Update,
        subsystem,
        request_context,
        input_context.as_ref(),
    )
    .await?;

    let arg_predicate = compute_predicate(
        &predicate_param.iter().collect::<Vec<_>>(),
        &field.arguments,
        subsystem,
        request_context,
    )
    .await?;
    let predicate = Predicate::and(entity_predicate, arg_predicate);

    match data_arg {
        Some(argument) => {
            let update = UpdateOperation {
                data_param,
                predicate,
                select,
                return_type,
            }
            .to_sql(argument, subsystem, request_context)
            .await?;

            Ok(OperationResolution {
                precheck_predicates: vec![precheck_predicate],
                operation: update,
            })
        }
        None => Err(PostgresExecutionError::MissingArgument(
            data_param.name.clone(),
        )),
    }
}
