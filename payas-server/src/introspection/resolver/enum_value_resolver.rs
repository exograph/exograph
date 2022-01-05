use async_graphql_parser::{
    types::{EnumValueDefinition, Field},
    Positioned,
};
use async_trait::async_trait;
use serde_json::Value;

use crate::execution::query_context::QueryContext;
use crate::execution::resolver::*;
use anyhow::{anyhow, Result};

#[async_trait(?Send)]
impl FieldResolver<Value> for EnumValueDefinition {
    async fn resolve_field<'e>(
        &'e self,
        _query_context: &'e QueryContext<'e>,
        field: &'e Positioned<Field>,
    ) -> Result<Value> {
        match field.node.name.node.as_str() {
            "name" => Ok(Value::String(self.value.node.as_str().to_owned())),
            "description" => Ok(self
                .description
                .clone()
                .map(|v| Value::String(v.node))
                .unwrap_or(Value::Null)),
            "isDeprecated" => Ok(Value::Bool(false)), // TODO
            "deprecationReason" => Ok(Value::Null),   // TODO
            "__typename" => Ok(Value::String("__EnumValue".to_string())),
            field_name => Err(anyhow!(GraphQLExecutionError::InvalidField(
                field_name.to_owned(),
                "EnumValueDefinition"
            ))),
        }
    }
}
