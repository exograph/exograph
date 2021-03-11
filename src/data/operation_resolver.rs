use crate::sql::ExpressionContext;
use crate::sql::{table::PhysicalTable, Expression};
use crate::{execution::query_context::QueryContext, sql::column::Column};

use crate::model::{operation::*, types::*};

use super::predicate_mapper::ArgumentSupplier;

use graphql_parser::{
    query::{Field, Selection},
    schema::Value,
};

use crate::{execution::query_context::QueryResponse, execution::resolver::OutputName};

use super::data_context::DataContext;

struct QueryParameters<'a, 'b> {
    predicate_arguments: Option<(&'a String, &'a Value<'b, String>)>,
    order_by_arguments: Option<(&'a String, &'a Value<'b, String>)>,
}

impl Query {
    pub fn resolve(
        &self,
        field: &Field<'_, String>,
        query_context: &QueryContext<'_>,
    ) -> QueryResponse {
        let string_response = self.operation(field, query_context);
        QueryResponse::Raw(string_response)
    }

    fn extract_arguments<'a, 'b>(
        &'a self,
        field: &'a Field<'b, String>,
    ) -> QueryParameters<'a, 'b> {
        let processed = field.arguments.iter().fold((None, None), |acc, argument| {
            let (argument_name, argument_value) = argument;

            if self
                .order_by_param
                .as_ref()
                .filter(|p| &p.name == argument_name)
                .is_some()
            {
                (acc.0, Some((argument_name, argument_value)))
            } else if self
                .predicate_parameter
                .as_ref()
                .filter(|p| &p.name == argument_name)
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

    fn operation(&self, field: &Field<'_, String>, query_context: &QueryContext<'_>) -> String {
        let table = self.physical_table(query_context.data_context).unwrap();
        let table_name = &table.name;

        let QueryParameters {
            predicate_arguments,
            order_by_arguments,
        } = self.extract_arguments(&field);

        let argument_supplier =
            predicate_arguments.map(|ps| ArgumentSupplier::new(ps.0.to_owned(), ps.1.to_owned()));

        let predicate = argument_supplier.as_ref().map(|argument_supplier| {
            let parameter = self
                .predicate_parameter
                .iter()
                .find(|p| &p.name == predicate_arguments.unwrap().0)
                .unwrap();

            parameter.predicate(
                &argument_supplier.argument_value,
                table,
                &query_context.data_context.system,
            )
        });

        let order_by = order_by_arguments.as_ref().map(|order_by_arguments| {
            let parameter = self
                .order_by_param
                .iter()
                .find(|p| &p.name == order_by_arguments.0)
                .unwrap();

            parameter.compute_order_by(
                order_by_arguments.1,
                table,
                &query_context.data_context.system,
            )
        });

        let content_object = self.content_select(field, table_name, query_context);

        let agg_column = Column::JsonAgg(&content_object);
        let single_column = vec![&content_object];
        let vector_column = vec![&agg_column];
        let single_select = table.select(&single_column, predicate.as_ref(), None);
        let vector_select = table.select(&vector_column, predicate.as_ref(), order_by);

        let mut expression_context = ExpressionContext::new();

        let binding = match self.return_type.type_modifier {
            ModelTypeModifier::Optional => single_select.binding(&mut expression_context),
            ModelTypeModifier::NonNull => single_select.binding(&mut expression_context),
            ModelTypeModifier::List => vector_select.binding(&mut expression_context),
        };

        query_context.data_context.database.execute(&binding)
    }

    fn content_select(
        &self,
        field: &Field<'_, String>,
        table_name: &str,
        query_context: &QueryContext<'_>,
    ) -> Column {
        let column_specs: Vec<_> = field
            .selection_set
            .items
            .iter()
            .flat_map(|selection| self.map_selection(selection, table_name, query_context))
            .collect();

        Column::JsonObject(column_specs)
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

    fn map_selection<'a, 'oc>(
        &self,
        selection: &Selection<'_, String>,
        table_name: &str,
        query_context: &QueryContext<'_>,
    ) -> Vec<(String, Column<'a>)> {
        match selection {
            Selection::Field(field) => {
                vec![self.map_field(field, table_name, query_context)]
            }
            Selection::FragmentSpread(fragment_spread) => {
                let fragment_definition =
                    query_context.fragment_definition(&fragment_spread).unwrap();
                fragment_definition
                    .selection_set
                    .items
                    .iter()
                    .flat_map(|selection| self.map_selection(selection, table_name, query_context))
                    .collect()
            }
            Selection::InlineFragment(_inline_fragment) => {
                vec![] // TODO
            }
        }
    }

    fn map_field<'a>(
        &self,
        field: &Field<'_, String>,
        table_name: &str,
        query_context: &QueryContext<'_>,
    ) -> (String, Column<'a>) {
        let return_type = query_context
            .data_context
            .system
            .find_type(&self.return_type.type_name)
            .unwrap();

        let model_field = return_type.model_field(&field.name).unwrap();

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
                let pk_query = query_context
                    .data_context
                    .system
                    .queries
                    .iter()
                    .find(|query| query.name == type_name.to_ascii_lowercase())
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
