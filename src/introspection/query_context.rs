use graphql_parser::query::{FragmentDefinition, Selection, SelectionSet};
use serde_json::{Map, Value};

use graphql_parser::{query::Field, schema::Type};
use resolver::*;
use util::*;

use super::{resolver::resolver, schema::Schema, util};

pub struct QueryContext<'a> {
    pub operation_name: &'a str,
    pub fragment_definitions: Vec<FragmentDefinition<'a, String>>,
    pub variables: &'a Option<&'a Map<String, Value>>,
    pub schema: &'a Schema<'a>,
}

impl<'qc> QueryContext<'qc> {
    pub fn resolve<'b>(&self, selection_set: &'b SelectionSet<'_, String>) -> Vec<(String, Value)> {
        selection_set
            .items
            .iter()
            .map(|selection| self.resolve_operation(selection))
            .collect::<Vec<(String, Value)>>()
    }

    fn resolve_operation<'b>(&self, selection: &'b Selection<'_, String>) -> (String, Value) {
        match selection {
            Selection::Field(field) => {
                if field.name == "__type" {
                    (field.output_name(), self.resolve_type(&field))
                } else if field.name == "__schema" {
                    (
                        field.output_name(),
                        self.schema.resolve_value(self, &field.selection_set),
                    )
                } else {
                    todo!()
                }
            }
            Selection::FragmentSpread(_fragment_spread) => ("unknown".to_string(), Value::Null),
            Selection::InlineFragment(_inline_fragment) => ("unknown".to_string(), Value::Null),
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
