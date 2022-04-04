use crate::{
    data::{
        operation_mapper::{compute_sql_access_predicate, SQLOperationKind},
        query_resolver::QuerySQLOperations,
    },
    execution::{operations_context::OperationsContext, resolver::GraphQLExecutionError},
    validation::field::ValidatedField,
};
use payas_sql::PhysicalTable;

use anyhow::{anyhow, bail, Context, Result};
use payas_model::model::{
    operation::{
        CreateDataParameter, Interceptors, Mutation, MutationKind, Query, UpdateDataParameter,
    },
    predicate::PredicateParameter,
    types::{GqlTypeKind, GqlTypeModifier},
};
use payas_sql::{
    AbstractDelete, AbstractInsert, AbstractOperation, AbstractPredicate, AbstractSelect,
    AbstractUpdate,
};

use super::operation_mapper::{
    OperationResolver, OperationResolverResult, SQLInsertMapper, SQLUpdateMapper,
};

impl<'a> OperationResolver<'a> for Mutation {
    fn resolve_operation(
        &'a self,
        field: &'a ValidatedField,
        query_context: &'a OperationsContext<'a>,
    ) -> Result<OperationResolverResult<'a>> {
        if let MutationKind::Service { method_id, .. } = &self.kind {
            Ok(OperationResolverResult::DenoOperation(method_id.unwrap()))
        } else {
            let abstract_select = {
                let (_, pk_query, collection_query) = return_type_info(self, query_context);
                let selection_query = match &self.return_type.type_modifier {
                    GqlTypeModifier::List => collection_query,
                    GqlTypeModifier::NonNull | GqlTypeModifier::Optional => pk_query,
                };

                selection_query.operation(field, AbstractPredicate::True, query_context)?
            };

            Ok(OperationResolverResult::SQLOperation(match &self.kind {
                MutationKind::Create(data_param) => AbstractOperation::Insert(create_operation(
                    self,
                    data_param,
                    field,
                    abstract_select,
                    query_context,
                )?),
                MutationKind::Delete(predicate_param) => AbstractOperation::Delete(
                    delete_operation(self, predicate_param, field, abstract_select, query_context)?,
                ),
                MutationKind::Update {
                    data_param,
                    predicate_param,
                } => AbstractOperation::Update(update_operation(
                    self,
                    data_param,
                    predicate_param,
                    field,
                    abstract_select,
                    query_context,
                )?),
                MutationKind::Service { .. } => panic!(),
            }))
        }
    }

    fn interceptors(&self) -> &Interceptors {
        &self.interceptors
    }

    fn name(&self) -> &str {
        &self.name
    }
}

fn create_operation<'a>(
    mutation: &'a Mutation,
    data_param: &'a CreateDataParameter,
    field: &'a ValidatedField,
    select: AbstractSelect<'a>,
    query_context: &'a OperationsContext<'a>,
) -> Result<AbstractInsert<'a>> {
    // TODO: https://github.com/payalabs/payas/issues/343
    let access_predicate = compute_sql_access_predicate(
        &mutation.return_type,
        &SQLOperationKind::Create,
        query_context,
    );

    // TODO: Allow access_predicate to have a residue that we can evaluate against data_param
    // See issue #69
    if access_predicate == AbstractPredicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        bail!(anyhow!(GraphQLExecutionError::Authorization))
    }

    let argument_value = super::find_arg(&field.arguments, &data_param.name).unwrap();

    data_param.insert_operation(mutation, select, argument_value, query_context)
}

fn delete_operation<'a>(
    mutation: &'a Mutation,
    predicate_param: &'a PredicateParameter,
    field: &'a ValidatedField,
    select: AbstractSelect<'a>,
    query_context: &'a OperationsContext<'a>,
) -> Result<AbstractDelete<'a>> {
    let (table, _, _) = return_type_info(mutation, query_context);

    // TODO: https://github.com/payalabs/payas/issues/343
    let access_predicate = compute_sql_access_predicate(
        &mutation.return_type,
        &SQLOperationKind::Delete,
        query_context,
    );

    if access_predicate == AbstractPredicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        bail!(anyhow!(GraphQLExecutionError::Authorization))
    }

    let predicate = super::compute_predicate(
        Some(predicate_param),
        &field.arguments,
        AbstractPredicate::True,
        query_context,
    )
    .with_context(|| {
        format!(
            "During predicate computation for parameter {}",
            predicate_param.name
        )
    })?;

    Ok(AbstractDelete {
        table,
        predicate: Some(predicate),
        selection: select,
    })
}

fn update_operation<'a>(
    mutation: &'a Mutation,
    data_param: &'a UpdateDataParameter,
    predicate_param: &'a PredicateParameter,
    field: &'a ValidatedField,
    select: AbstractSelect<'a>,
    query_context: &'a OperationsContext<'a>,
) -> Result<AbstractUpdate<'a>> {
    // Access control as well as predicate computation isn't working fully yet. Specifically,
    // nested predicates aren't working.
    // TODO: https://github.com/payalabs/payas/issues/343
    let access_predicate = compute_sql_access_predicate(
        &mutation.return_type,
        &SQLOperationKind::Update,
        query_context,
    );

    if access_predicate == AbstractPredicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        bail!(anyhow!(GraphQLExecutionError::Authorization))
    }

    // TODO: https://github.com/payalabs/payas/issues/343
    let predicate = super::compute_predicate(
        Some(predicate_param),
        &field.arguments,
        AbstractPredicate::True,
        query_context,
    )
    .with_context(|| {
        format!(
            "During predicate computation for parameter {}",
            predicate_param.name
        )
    })?;

    let argument_value = super::find_arg(&field.arguments, &data_param.name);
    argument_value
        .map(|argument_value| {
            data_param.update_operation(mutation, predicate, select, argument_value, query_context)
        })
        .unwrap()
}

pub fn return_type_info<'a>(
    mutation: &'a Mutation,
    query_context: &'a OperationsContext<'a>,
) -> (&'a PhysicalTable, &'a Query, &'a Query) {
    let system = &query_context.get_system();
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
