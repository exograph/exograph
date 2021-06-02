use crate::{
    data::query_resolver::QueryOperations,
    sql::{column::Column, predicate::Predicate, Cte, PhysicalTable, SQLOperation},
};

use payas_model::model::{operation::*, predicate::PredicateParameter, types::*};

use super::{
    operation_context::OperationContext,
    sql_mapper::{OperationResolver, SQLMapper},
};

use async_graphql_parser::{types::Field, Positioned};
use async_graphql_value::{Name, Value};

type Arguments = [(Positioned<Name>, Positioned<Value>)];

impl<'a> OperationResolver<'a> for Mutation {
    fn map_to_sql(
        &'a self,
        field: &'a Positioned<Field>,
        operation_context: &'a OperationContext<'a>,
    ) -> SQLOperation<'a> {
        let core_operation = match &self.kind {
            MutationKind::Create(data_param) => {
                create_operation(self, data_param, &field.node, &operation_context)
            }
            MutationKind::Delete(predicate_param) => {
                delete_operation(self, predicate_param, &field.node, &operation_context)
            }
            MutationKind::Update {
                data_param,
                predicate_param,
            } => update_operation(
                self,
                data_param,
                predicate_param,
                &field.node,
                &operation_context,
            ),
        };

        let (_, pk_query, collection_query) = return_type_info(self, operation_context);
        let selection_query = match &self.return_type.type_modifier {
            GqlTypeModifier::List => collection_query,
            GqlTypeModifier::NonNull | GqlTypeModifier::Optional => pk_query,
        };

        let select =
            selection_query.operation(&field.node, Predicate::True, operation_context, true);

        // Use the same name as the table in the select clause, since that is the name `pk_query.operation` uses
        let cte_name = format!("\"{}\"", select.underlying.name);

        SQLOperation::Cte(Cte {
            ctes: vec![(cte_name, core_operation)],
            select,
        })
    }
}

fn create_operation<'a>(
    mutation: &'a Mutation,
    data_param: &'a MutationDataParameter,
    field: &'a Field,
    operation_context: &'a OperationContext<'a>,
) -> SQLOperation<'a> {
    let (table, _, _) = return_type_info(mutation, operation_context);

    let column_values = data_columns(data_param, &field.arguments, operation_context).unwrap();

    SQLOperation::Insert(table.insert(
        column_values,
        vec![operation_context.create_column(Column::Star)],
    ))
}

fn delete_operation<'a>(
    mutation: &'a Mutation,
    predicate_param: &'a PredicateParameter,
    field: &'a Field,
    operation_context: &'a OperationContext<'a>,
) -> SQLOperation<'a> {
    let (table, _, _) = return_type_info(mutation, operation_context);

    let predicate = super::compute_predicate(
        Some(predicate_param),
        &field.arguments,
        Predicate::True,
        operation_context,
    );

    SQLOperation::Delete(table.delete(
        predicate,
        vec![operation_context.create_column(Column::Star)],
    ))
}

fn update_operation<'a>(
    mutation: &'a Mutation,
    data_param: &'a MutationDataParameter,
    predicate_param: &'a PredicateParameter,
    field: &'a Field,
    operation_context: &'a OperationContext<'a>,
) -> SQLOperation<'a> {
    let (table, _, _) = return_type_info(mutation, operation_context);

    let predicate = super::compute_predicate(
        Some(predicate_param),
        &field.arguments,
        Predicate::True,
        operation_context,
    )
    .unwrap();

    let column_values = data_columns(data_param, &field.arguments, operation_context).unwrap();

    SQLOperation::Update(table.update(
        column_values,
        predicate,
        vec![operation_context.create_column(Column::Star)],
    ))
}

fn data_columns<'a>(
    data_param: &'a MutationDataParameter,
    arguments: &'a Arguments,
    operation_context: &'a OperationContext<'a>,
) -> Option<Vec<(&'a Column<'a>, &'a Column<'a>)>> {
    let argument_value = super::find_arg(arguments, &data_param.name);
    argument_value.map(|argument_value| data_param.map_to_sql(argument_value, operation_context))
}

fn return_type_info<'a>(
    mutation: &'a Mutation,
    operation_context: &'a OperationContext<'a>,
) -> (&'a PhysicalTable, &'a Query, &'a Query) {
    let system = &operation_context.query_context.system;
    let typ = mutation.return_type.typ(system);

    match typ.kind {
        GqlTypeKind::Primitive => panic!(""),
        GqlTypeKind::Composite {
            table_id,
            pk_query,
            collection_query,
            ..
        } => (
            &system.tables[table_id],
            &system.queries[pk_query],
            &system.queries[collection_query],
        ),
    }
}
