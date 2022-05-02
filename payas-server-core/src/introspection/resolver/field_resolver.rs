use async_graphql_parser::types::FieldDefinition;
use async_trait::async_trait;
use serde_json::Value;

use crate::execution::resolver::{FieldResolver, GraphQLExecutionError, Resolver};
use crate::{execution::operations_context::OperationsContext, validation::field::ValidatedField};
use anyhow::{anyhow, Result};

#[async_trait]
impl FieldResolver<Value> for FieldDefinition {
    async fn resolve_field<'e>(
        &'e self,
        query_context: &'e OperationsContext<'e>,
        field: &ValidatedField,
    ) -> Result<Value> {
        match field.name.as_str() {
            "name" => Ok(Value::String(self.name.node.as_str().to_owned())),
            "description" => Ok(self
                .description
                .clone()
                .map(|v| Value::String(v.node))
                .unwrap_or(Value::Null)),
            "type" => self.ty.resolve_value(query_context, &field.subfields).await,
            "args" => {
                self.arguments
                    .resolve_value(query_context, &field.subfields)
                    .await
            }
            "isDeprecated" => Ok(Value::Bool(false)), // TODO
            "deprecationReason" => Ok(Value::Null),   // TODO
            "__typename" => Ok(Value::String("__Field".to_string())),
            field_name => Err(anyhow!(GraphQLExecutionError::InvalidField(
                field_name.to_owned(),
                "Field",
            ))),
        }
    }
}
