use graphql_parser::schema::EnumValue;
use serde_json::Value;

use crate::introspection::query_context;

use super::resolver::*;
use query_context::QueryContext;

impl<'a> FieldResolver for EnumValue<'a, String> {
    fn resolve_field(
        &self,
        _query_context: &QueryContext<'_>,
        field: &graphql_parser::query::Field<'_, String>,
    ) -> Value {
        match field.name.as_str() {
            "name" => Value::String(self.name.to_owned()),
            "description" => self
                .description
                .clone()
                .map(|v| Value::String(v))
                .unwrap_or(Value::Null),
            "isDeprecated" => Value::Bool(false), // TODO
            "deprecationReason" => Value::Null,   // TODO
            field_name => todo!("Invalid field {:?} for EnumValue", field_name), // TODO: Make it a proper error
        }
    }
}
