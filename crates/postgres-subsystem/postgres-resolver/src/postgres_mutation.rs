use async_trait::async_trait;
use core_resolver::{request_context::RequestContext, validation::field::ValidatedField};
use payas_sql::{
    AbstractDelete, AbstractInsert, AbstractOperation, AbstractSelect, AbstractUpdate,
};
use postgres_model::{
    model::ModelPostgresSystem,
    operation::{
        CreateDataParameter, OperationReturnType, PostgresMutation, PostgresMutationKind,
        UpdateDataParameter,
    },
    predicate::PredicateParameter,
    types::PostgresTypeModifier,
};

use crate::{
    create_data_param_mapper::InsertOperation, operation_resolver::OperationResolver,
    sql_mapper::SQLMapper, update_data_param_mapper::UpdateOperation,
};

use super::{
    postgres_execution_error::PostgresExecutionError,
    postgres_query::compute_select,
    sql_mapper::SQLOperationKind,
    util::{check_access, find_arg, return_type_info},
};

#[async_trait]
impl OperationResolver for PostgresMutation {
    async fn resolve<'a>(
        &'a self,
        field: &'a ValidatedField,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a ModelPostgresSystem,
    ) -> Result<AbstractOperation<'a>, PostgresExecutionError> {
        let return_type = &self.return_type;

        let abstract_select = {
            let (_, pk_query, collection_query) = return_type_info(return_type, subsystem);
            let selection_query = match return_type.type_modifier {
                PostgresTypeModifier::List => collection_query,
                PostgresTypeModifier::NonNull | PostgresTypeModifier::Optional => pk_query,
            };

            compute_select(selection_query, field, subsystem, request_context).await?
        };

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
    return_type: &'content OperationReturnType,
    data_param: &'content CreateDataParameter,
    field: &'content ValidatedField,
    select: AbstractSelect<'content>,
    subsystem: &'content ModelPostgresSystem,
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

    let argument = find_arg(&field.arguments, &data_param.name).unwrap();

    InsertOperation {
        data_param,
        select,
        return_type,
    }
    .to_sql(argument, subsystem)
}

async fn delete_operation<'content>(
    return_type: &'content OperationReturnType,
    predicate_param: &'content PredicateParameter,
    field: &'content ValidatedField,
    select: AbstractSelect<'content>,
    subsystem: &'content ModelPostgresSystem,
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

    let predicate = super::predicate_mapper::compute_predicate(
        Some(predicate_param),
        &field.arguments,
        subsystem,
    )?;

    Ok(AbstractDelete {
        table,
        predicate,
        selection: select,
    })
}

async fn update_operation<'content>(
    return_type: &'content OperationReturnType,
    data_param: &'content UpdateDataParameter,
    predicate_param: &'content PredicateParameter,
    field: &'content ValidatedField,
    select: AbstractSelect<'content>,
    subsystem: &'content ModelPostgresSystem,
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

    let predicate = super::predicate_mapper::compute_predicate(
        Some(predicate_param),
        &field.arguments,
        subsystem,
    )?;

    let argument = find_arg(&field.arguments, &data_param.name).unwrap();
    UpdateOperation {
        data_param,
        predicate,
        select,
        return_type,
    }
    .to_sql(argument, subsystem)
}
