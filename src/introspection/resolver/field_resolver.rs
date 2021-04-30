use async_graphql_parser::{
    types::{Field, FieldDefinition},
    Positioned,
};
use serde_json::Value;

use crate::execution::query_context::QueryContext;
use crate::execution::resolver::*;

impl FieldResolver<Value> for FieldDefinition {
    fn resolve_field(&self, query_context: &QueryContext<'_>, field: &Positioned<Field>) -> Value {
        match field.node.name.node.as_str() {
            "name" => Value::String(self.name.node.as_str().to_owned()),
            "description" => self
                .description
                .clone()
                .map(|v| Value::String(v.node))
                .unwrap_or(Value::Null),
            "type" => self
                .ty
                .resolve_value(query_context, &field.node.selection_set),
            "args" => self
                .arguments
                .resolve_value(query_context, &field.node.selection_set),
            "isDeprecated" => Value::Bool(false), // TODO
            "deprecationReason" => Value::Null,   // TODO
            field_name => {
                todo!("Invalid field {:?} for Field", field_name)
            } // TODO: Make it a proper error
        }
    }
}
