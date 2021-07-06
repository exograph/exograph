use async_graphql_parser::{
    types::{EnumValueDefinition, Field},
    Positioned,
};
use serde_json::Value;

use crate::execution::query_context::QueryContext;
use crate::execution::resolver::*;

impl FieldResolver<Value> for EnumValueDefinition {
    fn resolve_field(
        &self,
        _query_context: &QueryContext<'_>,
        field: &Positioned<Field>,
    ) -> Result<Value, GraphQLExecutionError> {
        match field.node.name.node.as_str() {
            "name" => Ok(Value::String(self.value.node.as_str().to_owned())),
            "description" => Ok(self
                .description
                .clone()
                .map(|v| Value::String(v.node))
                .unwrap_or(Value::Null)),
            "isDeprecated" => Ok(Value::Bool(false)), // TODO
            "deprecationReason" => Ok(Value::Null),   // TODO
            field_name => todo!("Invalid field {:?} for EnumValueDefinition", field_name), // TODO: Make it a proper error
        }
    }
}
