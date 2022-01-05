use async_graphql_parser::{
    types::{Field, InputValueDefinition},
    Positioned,
};
use async_trait::async_trait;
use serde_json::Value;

use crate::execution::query_context::QueryContext;
use crate::execution::resolver::*;
use anyhow::{anyhow, Result};

#[async_trait(?Send)]
impl FieldResolver<Value> for InputValueDefinition {
    async fn resolve_field<'e>(
        &'e self,
        query_context: &'e QueryContext<'e>,
        field: &'e Positioned<Field>,
    ) -> Result<Value> {
        match field.node.name.node.as_str() {
            "name" => Ok(Value::String(self.name.node.as_str().to_owned())),
            "description" => Ok(self
                .description
                .clone()
                .map(|v| Value::String(v.node))
                .unwrap_or(Value::Null)),
            "type" => {
                self.ty
                    .resolve_value(query_context, &field.node.selection_set)
                    .await
            }
            "defaultValue" => Ok(Value::Null), // TODO
            "__typename" => Ok(Value::String("__InputValue".to_string())),
            field_name => Err(anyhow!(GraphQLExecutionError::InvalidField(
                field_name.to_owned(),
                "InputValue",
            ))),
        }
    }
}
