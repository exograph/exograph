use crate::{
    model::{
        operation::*,
        predicate::PredicateParameter,
        types::{ModelTypeKind, ModelTypeModifier},
    },
    sql::{column::Column, predicate::Predicate, Cte, PhysicalTable, SQLOperation},
};

use super::operation_context::OperationContext;

use async_graphql_parser::{types::Field, Positioned};
use async_graphql_value::{Name, Value};

type Arguments = [(Positioned<Name>, Positioned<Value>)];

impl Mutation {
    pub fn resolve<'a>(
        &'a self,
        field: &'a Positioned<Field>,
        operation_context: &'a OperationContext<'a>,
    ) -> SQLOperation<'a> {
        let core_operation = match &self.kind {
            MutationKind::Create(data_param) => {
                self.create_operation(data_param, &field.node, &operation_context)
            }
            MutationKind::Delete(predicate_param) => {
                self.delete_operation(predicate_param, &field.node, &operation_context)
            }
            MutationKind::Update {
                data_param,
                predicate_param,
            } => {
                self.update_operation(data_param, predicate_param, &field.node, &operation_context)
            }
        };

        let (_, pk_query, collection_query) = self.return_type_info(operation_context);
        let selection_query = match &self.return_type.type_modifier {
            ModelTypeModifier::List => collection_query,
            ModelTypeModifier::NonNull | ModelTypeModifier::Optional => pk_query,
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

    fn create_operation<'a>(
        &'a self,
        data_param: &'a MutationDataParameter,
        field: &'a Field,
        operation_context: &'a OperationContext<'a>,
    ) -> SQLOperation<'a> {
        let (table, _, _) = self.return_type_info(operation_context);

        let column_values =
            Self::data_columns(data_param, &field.arguments, operation_context).unwrap();

        SQLOperation::Insert(table.insert(
            column_values,
            vec![operation_context.create_column(Column::Star)],
        ))
    }

    fn delete_operation<'a>(
        &'a self,
        predicate_param: &'a PredicateParameter,
        field: &'a Field,
        operation_context: &'a OperationContext<'a>,
    ) -> SQLOperation<'a> {
        let (table, _, _) = self.return_type_info(operation_context);

        let predicate = super::compute_predicate(
            &Some(predicate_param),
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
        &'a self,
        data_param: &'a MutationDataParameter,
        predicate_param: &'a PredicateParameter,
        field: &'a Field,
        operation_context: &'a OperationContext<'a>,
    ) -> SQLOperation<'a> {
        let (table, _, _) = self.return_type_info(operation_context);

        let predicate = super::compute_predicate(
            &Some(predicate_param),
            &field.arguments,
            Predicate::True,
            operation_context,
        )
        .unwrap();

        let column_values =
            Self::data_columns(data_param, &field.arguments, operation_context).unwrap();

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
        argument_value.map(|argument_value| {
            data_param.compute_mutation_data(argument_value, operation_context)
        })
    }

    fn return_type_info<'a>(
        &'a self,
        operation_context: &'a OperationContext<'a>,
    ) -> (&'a PhysicalTable, &'a Query, &'a Query) {
        let system = &operation_context.query_context.system;
        let typ = self.return_type.typ(system);

        match typ.kind {
            ModelTypeKind::Primitive => panic!(""),
            ModelTypeKind::Composite {
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
}
