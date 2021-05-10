use crate::{
    model::{
        predicate::PredicateParameter,
        types::{ModelTypeKind, ModelTypeModifier},
    },
    sql::{column::Column, predicate::Predicate, Cte, Delete, Insert, SQLOperation},
};

use crate::model::operation::*;

use super::operation_context::OperationContext;

use async_graphql_parser::{types::Field, Positioned};
use async_graphql_value::{Name, Value};

type Arguments = Vec<(Positioned<Name>, Positioned<Value>)>;

impl Mutation {
    pub fn resolve<'a>(
        &'a self,
        field: &'a Positioned<Field>,
        operation_context: &'a OperationContext<'a>,
    ) -> SQLOperation<'a> {
        match &self.kind {
            MutationKind::Create(data_param) => {
                self.create_operation(data_param, &field.node, &operation_context)
            }
            MutationKind::Delete(predicate_param) => {
                self.delete_operation(predicate_param, &field.node, &operation_context)
            }
        }
    }

    fn create_operation<'a>(
        &'a self,
        data_param: &'a MutationDataParameter,
        field: &'a Field,
        operation_context: &'a OperationContext<'a>,
    ) -> SQLOperation<'a> {
        let (table, pk_query) = {
            let system = &operation_context.query_context.system;
            let typ = self.return_type.typ(system);

            match typ.kind {
                ModelTypeKind::Primitive => panic!(""),
                ModelTypeKind::Composite {
                    table_id, pk_query, ..
                } => (&system.tables[table_id], &system.queries[pk_query]),
            }
        };

        let select = pk_query.operation(field, Predicate::True, operation_context, true);

        let insert = SQLOperation::Insert(Insert {
            underlying: table,
            column_values: Self::insertion_columns(data_param, &field.arguments, operation_context)
                .unwrap(),
            returning: vec![operation_context.create_column(Column::Star)],
        });

        // Use the same name as the table in the select clause, since that is the name `pk_query.operation` uses
        let cte_name = format!("\"{}\"", select.underlying.name);

        SQLOperation::Cte(Cte {
            ctes: vec![(cte_name, insert)],
            select,
        })
    }

    fn insertion_columns<'a>(
        data_param: &'a MutationDataParameter,
        arguments: &'a Arguments,
        operation_context: &'a OperationContext<'a>,
    ) -> Option<Vec<(&'a Column<'a>, &'a Column<'a>)>> {
        let argument_value = super::find_arg(arguments, &data_param.name);
        argument_value.map(|argument_value| {
            data_param.compute_mutation_data(argument_value, operation_context)
        })
    }

    fn delete_operation<'a>(
        &'a self,
        predicate_param: &'a PredicateParameter,
        field: &'a Field,
        operation_context: &'a OperationContext<'a>,
    ) -> SQLOperation<'a> {
        let (table, pk_query, collection_query) = {
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
        };

        let predicate = super::compute_predicate(
            &Some(predicate_param),
            &field.arguments,
            Predicate::True,
            operation_context,
        );

        let delete = SQLOperation::Delete(Delete {
            underlying: table,
            predicate,
            returning: vec![operation_context.create_column(Column::Star)],
        });

        let selection_query = match &self.return_type.type_modifier {
            ModelTypeModifier::List => collection_query,
            ModelTypeModifier::NonNull | ModelTypeModifier::Optional => pk_query,
        };

        let select = selection_query.operation(field, Predicate::True, operation_context, true);

        // Use the same name as the table in the select clause, since that is the name `pk_query.operation` uses
        let cte_name = format!("\"{}\"", select.underlying.name);

        SQLOperation::Cte(Cte {
            ctes: vec![(cte_name, delete)],
            select,
        })
    }
}
