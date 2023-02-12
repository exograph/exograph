use super::{
    postgres_execution_error::PostgresExecutionError,
    sql_mapper::SQLOperationKind,
    util::{check_access, find_arg, return_type_info},
};
use crate::{
    create_data_param_mapper::InsertOperation,
    operation_resolver::{OperationResolver, OperationSelectionResolver},
    predicate_mapper::compute_predicate,
    sql_mapper::SQLMapper,
    update_data_param_mapper::UpdateOperation,
};
use async_trait::async_trait;
use core_plugin_interface::core_model::types::OperationReturnType;
use core_plugin_interface::core_resolver::{
    request_context::RequestContext, validation::field::ValidatedField,
};
use payas_sql::{
    AbstractDelete, AbstractInsert, AbstractOperation, AbstractSelect, AbstractUpdate,
};
use postgres_model::{
    operation::{CreateDataParameter, PostgresMutation, PostgresMutationKind, UpdateDataParameter},
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
    ) -> Result<AbstractOperation<'a>, PostgresExecutionError> {
        let return_type = &self.return_type;

        let abstract_select = {
            let (_, pk_query, collection_query) = return_type_info(return_type, subsystem);
            match return_type {
                OperationReturnType::List(_) => {
                    collection_query.resolve_select(field, request_context, subsystem)
                }
                _ => pk_query.resolve_select(field, request_context, subsystem),
            }
        }
        .await?;

        Ok(match &self.kind {
            PostgresMutationKind::Create(data_param) => AbstractOperation::Insert(
                create_operation(
                    return_type,
                    data_param,
                    field,
                    abstract_select,
                    subsystem,
                    request_context,
                )
                .await?,
            ),
            PostgresMutationKind::Delete(predicate_param) => AbstractOperation::Delete(
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
            PostgresMutationKind::Update {
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
    return_type: &'content OperationReturnType<EntityType>,
    data_param: &'content CreateDataParameter,
    field: &'content ValidatedField,
    select: AbstractSelect<'content>,
    subsystem: &'content PostgresSubsystem,
    request_context: &'content RequestContext<'content>,
) -> Result<AbstractInsert<'content>, PostgresExecutionError> {
    // TODO: https://github.com/payalabs/payas/issues/343
    let _access_predicate = check_access(
        return_type,
        &SQLOperationKind::Create,
        subsystem,
        request_context,
    )
    .await?;

    match find_arg(&field.arguments, &data_param.name) {
        Some(argument) => InsertOperation {
            data_param,
            select,
            return_type,
        }
        .to_sql(argument, subsystem),
        None => Err(PostgresExecutionError::MissingArgument(
            data_param.name.clone(),
        )),
    }
}

async fn delete_operation<'content>(
    return_type: &'content OperationReturnType<EntityType>,
    predicate_param: &'content PredicateParameter,
    field: &'content ValidatedField,
    select: AbstractSelect<'content>,
    subsystem: &'content PostgresSubsystem,
    request_context: &'content RequestContext<'content>,
) -> Result<AbstractDelete<'content>, PostgresExecutionError> {
    let (table, _, _) = return_type_info(return_type, subsystem);

    // TODO: https://github.com/payalabs/payas/issues/343
    let _access_predicate = check_access(
        return_type,
        &SQLOperationKind::Delete,
        subsystem,
        request_context,
    )
    .await?;

    let predicate = compute_predicate(predicate_param, &field.arguments, subsystem)?;

    Ok(AbstractDelete {
        table,
        predicate,
        selection: select,
    })
}

async fn update_operation<'content>(
    return_type: &'content OperationReturnType<EntityType>,
    data_param: &'content UpdateDataParameter,
    predicate_param: &'content PredicateParameter,
    field: &'content ValidatedField,
    select: AbstractSelect<'content>,
    subsystem: &'content PostgresSubsystem,
    request_context: &'content RequestContext<'content>,
) -> Result<AbstractUpdate<'content>, PostgresExecutionError> {
    // Access control as well as predicate computation isn't working fully yet. Specifically,
    // nested predicates aren't working.
    // TODO: https://github.com/payalabs/payas/issues/343
    let _access_predicate = check_access(
        return_type,
        &SQLOperationKind::Update,
        subsystem,
        request_context,
    )
    .await?;

    let predicate = compute_predicate(predicate_param, &field.arguments, subsystem)?;

    match find_arg(&field.arguments, &data_param.name) {
        Some(argument) => UpdateOperation {
            data_param,
            predicate,
            select,
            return_type,
        }
        .to_sql(argument, subsystem),
        None => Err(PostgresExecutionError::MissingArgument(
            data_param.name.clone(),
        )),
    }
}
