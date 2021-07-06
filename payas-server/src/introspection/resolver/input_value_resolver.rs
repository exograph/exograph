use async_graphql_parser::{
    types::{Field, InputValueDefinition},
    Positioned,
};
use serde_json::Value;

use crate::execution::query_context::QueryContext;
use crate::execution::resolver::*;

impl FieldResolver<Value> for InputValueDefinition {
    fn resolve_field(
        &self,
        query_context: &QueryContext<'_>,
        field: &Positioned<Field>,
    ) -> Result<Value, GraphQLExecutionError> {
        match field.node.name.node.as_str() {
            "name" => Ok(Value::String(self.name.node.as_str().to_owned())),
            "description" => Ok(self
                .description
                .clone()
                .map(|v| Value::String(v.node))
                .unwrap_or(Value::Null)),
            "type" => self
                .ty
                .resolve_value(query_context, &field.node.selection_set),
            "defaultValue" => Ok(Value::Null), // TODO
            field_name => Err(GraphQLExecutionError::InvalidField(
                field_name.to_owned(),
                "InputValue",
            )),
        }
    }
}
