use crate::{
    model::types::ModelTypeKind,
    sql::{column::Column, predicate::Predicate, Cte, Insert, SQLOperation},
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
        SQLOperation::Cte(self.operation(&field.node, &operation_context))
    }

    fn operation<'a>(
        &'a self,
        field: &'a Field,
        operation_context: &'a OperationContext<'a>,
    ) -> Cte<'a> {
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
            column_values: self
                .insertion_columns(&field.arguments, operation_context)
                .unwrap(),
            returning: vec![operation_context.create_column(Column::Star)],
        });

        // Use the same name as the table in the select clause, since that is the name `pk_query.operation` uses
        let cte_name = format!("\"{}\"", select.underlying.name);

        Cte {
            ctes: vec![(cte_name, insert)],
            select,
        }
    }

    fn insertion_columns<'a>(
        &'a self,
        arguments: &'a Arguments,
        operation_context: &'a OperationContext<'a>,
    ) -> Option<Vec<(&Column, &Column)>> {
        match &self.kind {
            MutationKind::Create(data_param) => {
                let argument_value = super::find_arg(arguments, &data_param.name);
                argument_value.map(|argument_value| {
                    data_param.compute_mutation_data(argument_value, operation_context)
                })
            }
        }
    }
}
