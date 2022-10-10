use payas_core_resolver::{
    request_context::RequestContext, system_resolver::SystemResolver,
    validation::field::ValidatedField,
};
use payas_database_model::{
    model::ModelDatabaseSystem,
    operation::{CreateDataParameter, DatabaseMutation, DatabaseMutationKind, UpdateDataParameter},
    predicate::PredicateParameter,
    types::DatabaseTypeModifier,
};
use payas_sql::{
    AbstractDelete, AbstractInsert, AbstractOperation, AbstractPredicate, AbstractSelect,
    AbstractUpdate,
};

use super::{
    database_execution_error::{DatabaseExecutionError, WithContext},
    database_query::compute_select,
    sql_mapper::{SQLInsertMapper, SQLOperationKind, SQLUpdateMapper},
    util::{compute_sql_access_predicate, find_arg, return_type_info},
};

pub async fn operation<'content>(
    mutation: &'content DatabaseMutation,
    field: &'content ValidatedField,
    subsystem: &'content ModelDatabaseSystem,
    system_resolver: &'content SystemResolver,
    request_context: &'content RequestContext<'content>,
) -> Result<AbstractOperation<'content>, DatabaseExecutionError> {
    let abstract_select = {
        let return_type = &mutation.return_type;
        let (_, pk_query, collection_query) = return_type_info(return_type, subsystem);
        let selection_query = match return_type.type_modifier {
            DatabaseTypeModifier::List => collection_query,
            DatabaseTypeModifier::NonNull | DatabaseTypeModifier::Optional => pk_query,
        };

        compute_select(
            selection_query,
            field,
            AbstractPredicate::True,
            subsystem,
            system_resolver,
            request_context,
        )
        .await?
    };

    Ok(match &mutation.kind {
        DatabaseMutationKind::Create(data_param) => AbstractOperation::Insert(
            create_operation(
                mutation,
                data_param,
                field,
                abstract_select,
                subsystem,
                system_resolver,
                request_context,
            )
            .await?,
        ),
        DatabaseMutationKind::Delete(predicate_param) => AbstractOperation::Delete(
            delete_operation(
                mutation,
                predicate_param,
                field,
                abstract_select,
                subsystem,
                system_resolver,
                request_context,
            )
            .await?,
        ),
        DatabaseMutationKind::Update {
            data_param,
            predicate_param,
        } => AbstractOperation::Update(
            update_operation(
                mutation,
                data_param,
                predicate_param,
                field,
                abstract_select,
                subsystem,
                system_resolver,
                request_context,
            )
            .await?,
        ),
    })
}

async fn create_operation<'content>(
    mutation: &'content DatabaseMutation,
    data_param: &'content CreateDataParameter,
    field: &'content ValidatedField,
    select: AbstractSelect<'content>,
    subsystem: &'content ModelDatabaseSystem,
    system_resolver: &'content SystemResolver,
    request_context: &'content RequestContext<'content>,
) -> Result<AbstractInsert<'content>, DatabaseExecutionError> {
    // TODO: https://github.com/payalabs/payas/issues/343
    let access_predicate = compute_sql_access_predicate(
        &mutation.return_type,
        &SQLOperationKind::Create,
        subsystem,
        system_resolver,
        request_context,
    )
    .await;

    // TODO: Allow access_predicate to have a residue that we can evaluate against data_param
    // See issue #69
    if access_predicate == AbstractPredicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        return Err(DatabaseExecutionError::Authorization);
    }

    let argument_value = find_arg(&field.arguments, &data_param.name).unwrap();

    data_param.insert_operation(
        mutation.return_type.clone(),
        select,
        argument_value,
        subsystem,
        system_resolver,
    )
}

async fn delete_operation<'content>(
    mutation: &'content DatabaseMutation,
    predicate_param: &'content PredicateParameter,
    field: &'content ValidatedField,
    select: AbstractSelect<'content>,
    subsystem: &'content ModelDatabaseSystem,
    system_resolver: &'content SystemResolver,
    request_context: &'content RequestContext<'content>,
) -> Result<AbstractDelete<'content>, DatabaseExecutionError> {
    let (table, _, _) = return_type_info(&mutation.return_type, subsystem);

    // TODO: https://github.com/payalabs/payas/issues/343
    let access_predicate = compute_sql_access_predicate(
        &mutation.return_type,
        &SQLOperationKind::Delete,
        subsystem,
        system_resolver,
        request_context,
    )
    .await;

    if access_predicate == AbstractPredicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        return Err(DatabaseExecutionError::Authorization);
    }

    let predicate = super::predicate_mapper::compute_predicate(
        Some(predicate_param),
        &field.arguments,
        AbstractPredicate::True,
        subsystem,
        system_resolver,
    )
    .with_context(format!(
        "During predicate computation for parameter {}",
        predicate_param.name
    ))?;

    Ok(AbstractDelete {
        table,
        predicate: Some(predicate),
        selection: select,
    })
}

async fn update_operation<'content>(
    mutation: &'content DatabaseMutation,
    data_param: &'content UpdateDataParameter,
    predicate_param: &'content PredicateParameter,
    field: &'content ValidatedField,
    select: AbstractSelect<'content>,
    subsystem: &'content ModelDatabaseSystem,
    system_resolver: &'content SystemResolver,
    request_context: &'content RequestContext<'content>,
) -> Result<AbstractUpdate<'content>, DatabaseExecutionError> {
    // Access control as well as predicate computation isn't working fully yet. Specifically,
    // nested predicates aren't working.
    // TODO: https://github.com/payalabs/payas/issues/343
    let access_predicate = compute_sql_access_predicate(
        &mutation.return_type,
        &SQLOperationKind::Update,
        subsystem,
        system_resolver,
        request_context,
    )
    .await;

    if access_predicate == AbstractPredicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        return Err(DatabaseExecutionError::Authorization);
    }

    // TODO: https://github.com/payalabs/payas/issues/343
    let predicate = super::predicate_mapper::compute_predicate(
        Some(predicate_param),
        &field.arguments,
        AbstractPredicate::True,
        subsystem,
        system_resolver,
    )
    .with_context(format!(
        "During predicate computation for parameter {}",
        predicate_param.name
    ))?;

    let argument_value = find_arg(&field.arguments, &data_param.name);
    argument_value
        .map(|argument_value| {
            data_param.update_operation(
                &mutation.return_type,
                predicate,
                select,
                argument_value,
                subsystem,
                system_resolver,
            )
        })
        .unwrap()
}
