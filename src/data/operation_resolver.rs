use crate::execution::query_context::QueryContext;
use crate::sql::{
    column::Column,
    predicate::Predicate,
    table::{PhysicalTable, SelectionTable},
    Expression, ExpressionContext,
};

use crate::model::{operation::*, types::*};

use super::operation_context::OperationContext;

use async_graphql_parser::{
    types::{Field, Selection},
    Positioned,
};
use async_graphql_value::{Name, Value};

use crate::{execution::query_context::QueryResponse, execution::resolver::OutputName};

use super::data_context::DataContext;

struct QueryParameters<'a> {
    predicate_arguments: Option<(&'a Positioned<Name>, &'a Positioned<Value>)>,
    order_by_arguments: Option<(&'a Positioned<Name>, &'a Positioned<Value>)>,
}

impl Query {
    pub fn resolve(
        &self,
        field: &Positioned<Field>,
        query_context: &QueryContext<'_>,
    ) -> QueryResponse {
        let operation_context = OperationContext::new(query_context);
        let selection_table = self.operation(&field.node, &operation_context);
        let mut expression_context = ExpressionContext::new();
        let binding = selection_table.binding(&mut expression_context);
        let string_response = query_context.data_context.database.execute(&binding);
        QueryResponse::Raw(string_response)
    }

    fn extract_arguments<'a>(&'a self, field: &'a Field) -> QueryParameters<'a> {
        let processed = field.arguments.iter().fold((None, None), |acc, argument| {
            let (argument_name, argument_value) = argument;

            if self
                .order_by_param
                .as_ref()
                .filter(|p| p.name == argument_name.node)
                .is_some()
            {
                (acc.0, Some((argument_name, argument_value)))
            } else if self
                .predicate_parameter
                .as_ref()
                .filter(|p| p.name == argument_name.node)
                .is_some()
            {
                (Some((argument_name, argument_value)), acc.1)
            } else {
                todo!()
            }
        });

        QueryParameters {
            predicate_arguments: processed.0,
            order_by_arguments: processed.1,
        }
    }

    fn compute_predicate<'a>(
        &self,
        field: &'a Field,
        operation_context: &'a OperationContext<'a>,
    ) -> Option<Predicate<'a>> {
        let table = self
            .physical_table(operation_context.query_context.data_context)
            .unwrap();

        self.predicate_parameter
            .as_ref()
            .and_then(|predicate_parameter| {
                field.arguments.iter().find_map(|argument| {
                    let (argument_name, argument_value) = argument;
                    if predicate_parameter.name == argument_name.node {
                        Some(predicate_parameter.predicate(
                            &argument_value.node,
                            table,
                            operation_context,
                        ))
                    } else {
                        None
                    }
                })
            })
    }

    fn operation<'a>(
        &'a self,
        field: &'a Field,
        operation_context: &'a OperationContext<'a>,
    ) -> SelectionTable<'a> {
        let table = self
            .physical_table(operation_context.query_context.data_context)
            .unwrap();

        let QueryParameters {
            predicate_arguments,
            order_by_arguments,
        } = self.extract_arguments(&field);

        let predicate = predicate_arguments
            .map(|predicate_arguments| {
                let parameter = self
                    .predicate_parameter
                    .iter()
                    .find(|p| p.name == predicate_arguments.0.node)
                    .unwrap();
                parameter.predicate(&predicate_arguments.1.node, table, operation_context)
            })
            .map(|p| operation_context.create_predicate(p));

        let order_by = order_by_arguments.map(|order_by_arguments| {
            let parameter = self
                .order_by_param
                .iter()
                .find(|p| p.name == order_by_arguments.0.node)
                .unwrap();

            parameter.compute_order_by(
                &order_by_arguments.1.node,
                table,
                &operation_context.query_context.data_context.system,
            )
        });

        let content_object = self.content_select(field, &operation_context);

        match self.return_type.type_modifier {
            ModelTypeModifier::Optional | ModelTypeModifier::NonNull => {
                let single_column = vec![content_object];
                table.select(single_column, predicate, None)
            }
            ModelTypeModifier::List => {
                let agg_column = operation_context.create_column(Column::JsonAgg(content_object));
                let vector_column = vec![agg_column];
                table.select(vector_column, predicate, order_by)
            }
        }
    }

    fn content_select<'a>(
        &self,
        field: &Field,
        operation_context: &'a OperationContext<'a>,
    ) -> &'a Column<'a> {
        let table = self
            .physical_table(operation_context.query_context.data_context)
            .unwrap();
        let table_name = &table.name;

        let column_specs: Vec<_> = field
            .selection_set
            .node
            .items
            .iter()
            .flat_map(|selection| {
                self.map_selection(&selection.node, table_name, &operation_context)
            })
            .collect();

        operation_context.create_column(Column::JsonObject(column_specs))
    }

    fn physical_table<'a>(&self, data_context: &'a DataContext) -> Option<&'a PhysicalTable<'a>> {
        data_context
            .system
            .find_type(&self.return_type.type_name)
            .map(|t| match &t.kind {
                ModelTypeKind::Composite {
                    model_fields: _,
                    table_name,
                } => Some(table_name),
                _ => None,
            })
            .flatten()
            .map(|table_name| data_context.database.get_table(table_name))
            .flatten()
    }

    fn map_selection<'a>(
        &self,
        selection: &Selection,
        table_name: &str,
        operation_context: &OperationContext,
    ) -> Vec<(String, Column<'a>)> {
        match selection {
            Selection::Field(field) => {
                vec![self.map_field(&field.node, table_name, &operation_context)]
            }
            Selection::FragmentSpread(fragment_spread) => {
                let fragment_definition = operation_context
                    .query_context
                    .fragment_definition(&fragment_spread)
                    .unwrap();
                fragment_definition
                    .selection_set
                    .node
                    .items
                    .iter()
                    .flat_map(|selection| {
                        self.map_selection(&selection.node, table_name, &operation_context)
                    })
                    .collect()
            }
            Selection::InlineFragment(_inline_fragment) => {
                vec![] // TODO
            }
        }
    }

    fn map_field<'a>(
        &self,
        field: &Field,
        table_name: &str,
        operation_context: &OperationContext,
    ) -> (String, Column<'a>) {
        let return_type = operation_context
            .query_context
            .data_context
            .system
            .find_type(&self.return_type.type_name)
            .unwrap();

        let model_field = return_type.model_field(&field.name.node).unwrap();

        let column = match &model_field.relation {
            ModelRelation::Pk { .. } | ModelRelation::Scalar { .. } => Column::Physical {
                table_name: table_name.to_string(),
                column_name: model_field.column_name(),
            },
            ModelRelation::ManyToOne {
                column_name,
                type_name,
                optional: _,
            } => {
                let pk_query = operation_context
                    .query_context
                    .data_context
                    .system
                    .queries
                    .iter()
                    .find(|query| query.name == type_name.to_ascii_lowercase()) // TODO: Implement a systematic way
                    .unwrap();
                // TODO: Use column_name to create a predicate....
                //pk_query.content_select(field, additional_predicate, table_name, operation_context)
                todo!()
            }
            ModelRelation::OneToMany {
                column_name: _,
                type_name: _,
                optional: _,
            } => todo!(),
        };

        (field.output_name(), column)
    }
}
