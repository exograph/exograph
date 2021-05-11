use async_graphql_parser::{Positioned, types::{EnumValueDefinition, Field}};
use serde_json::Value;

use crate::execution::query_context::QueryContext;
use crate::execution::resolver::*;

impl FieldResolver<Value> for EnumValueDefinition {
    fn resolve_field(&self, _query_context: &QueryContext<'_>, field: &Positioned<Field>) -> Value {
        match field.node.name.node.as_str() {
            "name" => Value::String(self.value.node.as_str().to_owned()),
            "description" => self
                .description
                .clone()
                .map(|v| Value::String(v.node))
                .unwrap_or(Value::Null),
            "isDeprecated" => Value::Bool(false), // TODO
            "deprecationReason" => Value::Null,   // TODO
            field_name => todo!("Invalid field {:?} for EnumValueDefinition", field_name), // TODO: Make it a proper error
        }
    }
}
