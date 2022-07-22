use crate::{
    data::{compute_sql_access_predicate, operation_mapper::SQLOperationKind},
    execution::system_context::SystemContext,
    execution_error::{ExecutionError, WithContext},
    request_context::RequestContext,
    resolver::OperationResolver,
    validation::field::ValidatedField,
};
use async_trait::async_trait;
use payas_sql::PhysicalTable;

use payas_model::model::{
    operation::{
        CreateDataParameter, DatabaseMutationKind, Interceptors, Mutation, MutationKind, Query,
        UpdateDataParameter,
    },
    predicate::PredicateParameter,
    types::{GqlTypeKind, GqlTypeModifier},
};
use payas_sql::{
    AbstractDelete, AbstractInsert, AbstractOperation, AbstractPredicate, AbstractSelect,
    AbstractUpdate,
};

use super::{
    operation_mapper::{DenoOperation, OperationResolverResult, SQLInsertMapper, SQLUpdateMapper},
    query_resolver::DatabaseQuery,
};

#[async_trait]
impl<'a> OperationResolver<'a> for Mutation {
    async fn resolve_operation(
        &'a self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<OperationResolverResult<'a>, ExecutionError> {
        match &self.kind {
            MutationKind::Database { kind } => {
                let abstract_select = {
                    let (_, pk_query, collection_query) = return_type_info(self, system_context);
                    let selection_query = match &self.return_type.type_modifier {
                        GqlTypeModifier::List => collection_query,
                        GqlTypeModifier::NonNull | GqlTypeModifier::Optional => pk_query,
                    };

                    DatabaseQuery::from(selection_query)
                        .operation(
                            field,
                            AbstractPredicate::True,
                            system_context,
                            request_context,
                        )
                        .await?
                };

                Ok(OperationResolverResult::SQLOperation(match kind {
                    DatabaseMutationKind::Create(data_param) => AbstractOperation::Insert(
                        create_operation(
                            self,
                            data_param,
                            field,
                            abstract_select,
                            system_context,
                            request_context,
                        )
                        .await?,
                    ),
                    DatabaseMutationKind::Delete(predicate_param) => AbstractOperation::Delete(
                        delete_operation(
                            self,
                            predicate_param,
                            field,
                            abstract_select,
                            system_context,
                            request_context,
                        )
                        .await?,
                    ),
                    DatabaseMutationKind::Update {
                        data_param,
                        predicate_param,
                    } => AbstractOperation::Update(
                        update_operation(
                            self,
                            data_param,
                            predicate_param,
                            field,
                            abstract_select,
                            system_context,
                            request_context,
                        )
                        .await?,
                    ),
                }))
            }

            MutationKind::Service { method_id, .. } => Ok(OperationResolverResult::DenoOperation(
                DenoOperation(method_id.unwrap()),
            )),
        }
    }

    fn interceptors(&self) -> &Interceptors {
        &self.interceptors
    }

    fn name(&self) -> &str {
        &self.name
    }
}

async fn create_operation<'a>(
    mutation: &'a Mutation,
    data_param: &'a CreateDataParameter,
    field: &'a ValidatedField,
    select: AbstractSelect<'a>,
    system_context: &'a SystemContext,
    request_context: &'a RequestContext<'a>,
) -> Result<AbstractInsert<'a>, ExecutionError> {
    // TODO: https://github.com/payalabs/payas/issues/343
    let access_predicate = compute_sql_access_predicate(
        &mutation.return_type,
        &SQLOperationKind::Create,
        system_context,
        request_context,
    )
    .await;

    // TODO: Allow access_predicate to have a residue that we can evaluate against data_param
    // See issue #69
    if access_predicate == AbstractPredicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        return Err(ExecutionError::Authorization);
    }

    let argument_value = super::find_arg(&field.arguments, &data_param.name).unwrap();

    data_param.insert_operation(mutation, select, argument_value, system_context)
}

async fn delete_operation<'a>(
    mutation: &'a Mutation,
    predicate_param: &'a PredicateParameter,
    field: &'a ValidatedField,
    select: AbstractSelect<'a>,
    system_context: &'a SystemContext,
    request_context: &'a RequestContext<'a>,
) -> Result<AbstractDelete<'a>, ExecutionError> {
    let (table, _, _) = return_type_info(mutation, system_context);

    // TODO: https://github.com/payalabs/payas/issues/343
    let access_predicate = compute_sql_access_predicate(
        &mutation.return_type,
        &SQLOperationKind::Delete,
        system_context,
        request_context,
    )
    .await;

    if access_predicate == AbstractPredicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        return Err(ExecutionError::Authorization);
    }

    let predicate = super::compute_predicate(
        Some(predicate_param),
        &field.arguments,
        AbstractPredicate::True,
        system_context,
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

async fn update_operation<'a>(
    mutation: &'a Mutation,
    data_param: &'a UpdateDataParameter,
    predicate_param: &'a PredicateParameter,
    field: &'a ValidatedField,
    select: AbstractSelect<'a>,
    system_context: &'a SystemContext,
    request_context: &'a RequestContext<'a>,
) -> Result<AbstractUpdate<'a>, ExecutionError> {
    // Access control as well as predicate computation isn't working fully yet. Specifically,
    // nested predicates aren't working.
    // TODO: https://github.com/payalabs/payas/issues/343
    let access_predicate = compute_sql_access_predicate(
        &mutation.return_type,
        &SQLOperationKind::Update,
        system_context,
        request_context,
    )
    .await;

    if access_predicate == AbstractPredicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        return Err(ExecutionError::Authorization);
    }

    // TODO: https://github.com/payalabs/payas/issues/343
    let predicate = super::compute_predicate(
        Some(predicate_param),
        &field.arguments,
        AbstractPredicate::True,
        system_context,
    )
    .with_context(format!(
        "During predicate computation for parameter {}",
        predicate_param.name
    ))?;

    let argument_value = super::find_arg(&field.arguments, &data_param.name);
    argument_value
        .map(|argument_value| {
            data_param.update_operation(mutation, predicate, select, argument_value, system_context)
        })
        .unwrap()
}

pub fn return_type_info<'a>(
    mutation: &'a Mutation,
    system_context: &'a SystemContext,
) -> (&'a PhysicalTable, &'a Query, &'a Query) {
    let system = &system_context.system;
    let typ = mutation.return_type.typ(system);

    match &typ.kind {
        GqlTypeKind::Primitive => panic!(""),
        GqlTypeKind::Composite(kind) => (
            &system.tables[kind.get_table_id()],
            &system.queries[kind.get_pk_query()],
            &system.queries[kind.get_collection_query()],
        ),
    }
}
