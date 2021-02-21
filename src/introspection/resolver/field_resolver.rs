use graphql_parser::schema::Field;
use serde_json::Value;

use crate::execution::query_context::QueryContext;
use crate::execution::resolver::*;

impl<'a> FieldResolver for Field<'a, String> {
    fn resolve_field(
        &self,
        query_context: &QueryContext<'_>,
        field: &graphql_parser::query::Field<'_, String>,
    ) -> Value {
        match field.name.as_str() {
            "name" => Value::String(self.name.to_owned()),
            "description" => self
                .description
                .clone()
                .map(|v| Value::String(v))
                .unwrap_or(Value::Null),
            "type" => self
                .field_type
                .resolve_value(query_context, &field.selection_set),
            "args" => self
                .arguments
                .resolve_value(query_context, &field.selection_set),
            "isDeprecated" => Value::Bool(false), // TODO
            "deprecationReason" => Value::Null,   // TODO
            field_name => todo!("Invalid field {:?} for Field", field_name), // TODO: Make it a proper error
        }
    }
}
