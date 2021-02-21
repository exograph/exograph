use crate::DataContext;
use graphql_parser::query::{FragmentDefinition, Selection, SelectionSet};
use serde_json::{Map, Value};

use super::resolver::*;
use graphql_parser::{query::Field, schema::Type};

use crate::introspection::schema::Schema;

pub struct QueryContext<'a> {
    pub operation_name: &'a str,
    pub fragment_definitions: Vec<FragmentDefinition<'a, String>>,
    pub variables: &'a Option<&'a Map<String, Value>>,
    pub schema: &'a Schema<'a>,
    pub data_system: &'a DataContext<'a>,
}

pub enum QueryResponse {
    Json(Value),
    Raw(String),
}

impl<'qc> QueryContext<'qc> {
    pub fn resolve<'b>(
        &self,
        selection_set: &'b SelectionSet<'_, String>,
    ) -> Vec<(String, QueryResponse)> {
        selection_set
            .items
            .iter()
            .map(|selection| self.resolve_operation(selection))
            .collect::<Vec<(String, QueryResponse)>>()
    }

    fn resolve_operation<'b>(
        &self,
        selection: &'b Selection<'_, String>,
    ) -> (String, QueryResponse) {
        match selection {
            Selection::Field(field) => {
                if field.name == "__type" {
                    (
                        field.output_name(),
                        QueryResponse::Json(self.resolve_type(&field)),
                    )
                } else if field.name == "__schema" {
                    (
                        field.output_name(),
                        QueryResponse::Json(self.schema.resolve_value(self, &field.selection_set)),
                    )
                } else {
                    (field.output_name(), self.data_system.resolve(&field))
                }
            }
            Selection::FragmentSpread(_fragment_spread) => todo!(),
            Selection::InlineFragment(_inline_fragment) => todo!(),
        }
    }

    fn resolve_type(&self, field: &Field<'_, String>) -> Value {
        let name_arg = field.arguments.iter().find(|arg| arg.0 == "name").unwrap();

        if let graphql_parser::query::Value::String(name_specified) = &name_arg.1 {
            let tpe: Type<String> = Type::NamedType(name_specified.to_owned());
            tpe.resolve_value(self, &field.selection_set)
        } else {
            Value::Null
        }
    }
}
