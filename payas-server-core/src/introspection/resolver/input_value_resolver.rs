use async_graphql_parser::types::InputValueDefinition;
use async_trait::async_trait;
use serde_json::Value;

use crate::execution::resolver::{FieldResolver, GraphQLExecutionError, Resolver};
use crate::request_context::RequestContext;
use crate::{execution::operations_context::OperationsContext, validation::field::ValidatedField};
use anyhow::{anyhow, Result};

#[async_trait]
impl FieldResolver<Value> for InputValueDefinition {
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        operations_context: &'e OperationsContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value> {
        match field.name.as_str() {
            "name" => Ok(Value::String(self.name.node.as_str().to_owned())),
            "description" => Ok(self
                .description
                .clone()
                .map(|v| Value::String(v.node))
                .unwrap_or(Value::Null)),
            "type" => {
                self.ty
                    .resolve_value(&field.subfields, operations_context, request_context)
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
