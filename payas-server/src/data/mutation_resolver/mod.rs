use crate::{
    data::{
        operation_mapper::{compute_sql_access_predicate, SQLOperationKind},
        query_resolver::QuerySQLOperations,
    },
    execution::resolver::GraphQLExecutionError,
    sql::{column::Column, predicate::Predicate, Cte, PhysicalTable, SQLOperation},
};

use anyhow::*;
use payas_model::{
    model::{operation::*, predicate::PredicateParameter, types::*},
    sql::{
        transaction::{ConcreteTransactionStep, TransactionScript, TransactionStep},
        Select,
    },
};

use super::{
    create_data_param_mapper::InsertionInfo,
    operation_context::OperationContext,
    operation_mapper::{OperationResolver, OperationResolverResult, SQLMapper, SQLUpdateMapper},
};

use async_graphql_parser::{types::Field, Positioned};
use async_graphql_value::{Name, Value};

type Arguments = [(Positioned<Name>, Positioned<Value>)];

impl<'a> OperationResolver<'a> for Mutation {
    fn resolve_operation(
        &'a self,
        field: &'a Positioned<Field>,
        operation_context: &'a OperationContext<'a>,
    ) -> Result<OperationResolverResult<'a>> {
        if let MutationKind::Service { method_id, .. } = &self.kind {
            Ok(OperationResolverResult::DenoOperation(method_id.unwrap()))
        } else {
            let select = {
                let (_, pk_query, collection_query) = return_type_info(self, operation_context);
                let selection_query = match &self.return_type.type_modifier {
                    GqlTypeModifier::List => collection_query,
                    GqlTypeModifier::NonNull | GqlTypeModifier::Optional => pk_query,
                };

                selection_query.operation(&field.node, Predicate::True, operation_context, true)?
            };

            Ok(OperationResolverResult::SQLOperation(match &self.kind {
                MutationKind::Create(data_param) => {
                    create_operation(self, data_param, &field.node, select, operation_context)?
                }
                MutationKind::Delete(predicate_param) => delete_operation(
                    self,
                    predicate_param,
                    &field.node,
                    select,
                    operation_context,
                )?,
                MutationKind::Update {
                    data_param,
                    predicate_param,
                } => update_operation(
                    self,
                    data_param,
                    predicate_param,
                    &field.node,
                    select,
                    operation_context,
                )?,
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

pub fn table_name(mutation: &Mutation, operation_context: &OperationContext) -> String {
    mutation
        .return_type
        .physical_table(operation_context.get_system())
        .name
        .to_owned()
}

fn create_operation<'a>(
    mutation: &'a Mutation,
    data_param: &'a CreateDataParameter,
    field: &'a Field,
    select: Select<'a>,
    operation_context: &'a OperationContext<'a>,
) -> Result<TransactionScript<'a>> {
    let access_predicate = compute_sql_access_predicate(
        &mutation.return_type,
        &SQLOperationKind::Create,
        operation_context,
    );

    // TODO: Allow access_predicate to have a residue that we can evaluate against data_param
    // See issue #69
    if access_predicate == &Predicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        bail!(anyhow!(GraphQLExecutionError::Authorization))
    }

    let info = insertion_info(data_param, &field.arguments, operation_context)?.unwrap();
    let ops = info.operation(operation_context, true);

    Ok(TransactionScript::Single(TransactionStep::Concrete(
        ConcreteTransactionStep::new(SQLOperation::Cte(Cte { ctes: ops, select })),
    )))
}

fn delete_operation<'a>(
    mutation: &'a Mutation,
    predicate_param: &'a PredicateParameter,
    field: &'a Field,
    select: Select<'a>,
    operation_context: &'a OperationContext<'a>,
) -> Result<TransactionScript<'a>> {
    let (table, _, _) = return_type_info(mutation, operation_context);

    let access_predicate = compute_sql_access_predicate(
        &mutation.return_type,
        &SQLOperationKind::Delete,
        operation_context,
    );

    if access_predicate == &Predicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        bail!(anyhow!(GraphQLExecutionError::Authorization))
    }

    let predicate = super::compute_predicate(
        Some(predicate_param),
        &field.arguments,
        access_predicate.into(),
        operation_context,
    )
    .with_context(|| {
        format!(
            "During predicate computation for parameter {}",
            predicate_param.name
        )
    })?;

    let ops = vec![(
        table_name(mutation, operation_context),
        SQLOperation::Delete(table.delete(Some(predicate), vec![Column::Star.into()])),
    )];

    Ok(TransactionScript::Single(TransactionStep::Concrete(
        ConcreteTransactionStep::new(SQLOperation::Cte(Cte { ctes: ops, select })),
    )))
}

fn update_operation<'a>(
    mutation: &'a Mutation,
    data_param: &'a UpdateDataParameter,
    predicate_param: &'a PredicateParameter,
    field: &'a Field,
    select: Select<'a>,
    operation_context: &'a OperationContext<'a>,
) -> Result<TransactionScript<'a>> {
    let access_predicate = compute_sql_access_predicate(
        &mutation.return_type,
        &SQLOperationKind::Update,
        operation_context,
    );

    if access_predicate == &Predicate::False {
        // Hard failure, no need to proceed to restrict the predicate in SQL
        bail!(anyhow!(GraphQLExecutionError::Authorization))
    }

    let predicate = super::compute_predicate(
        Some(predicate_param),
        &field.arguments,
        Predicate::True.into(),
        operation_context,
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
            data_param.update_script(
                mutation,
                predicate,
                select,
                argument_value,
                operation_context,
            )
        })
        .unwrap()
}

fn insertion_info<'a>(
    data_param: &'a CreateDataParameter,
    arguments: &'a Arguments,
    operation_context: &'a OperationContext<'a>,
) -> Result<Option<InsertionInfo<'a>>> {
    let system = &operation_context.get_system();
    let data_type = &system.mutation_types[data_param.type_id];

    let argument_value = super::find_arg(arguments, &data_param.name);
    argument_value
        .map(|argument_value| data_type.map_to_sql(argument_value, operation_context))
        .transpose()
}

pub fn return_type_info<'a>(
    mutation: &'a Mutation,
    operation_context: &'a OperationContext<'a>,
) -> (&'a PhysicalTable, &'a Query, &'a Query) {
    let system = &operation_context.get_system();
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
