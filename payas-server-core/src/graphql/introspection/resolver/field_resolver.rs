use async_graphql_parser::types::FieldDefinition;
use async_trait::async_trait;
use serde_json::Value;

use crate::graphql::execution::resolver::{FieldResolver, Resolver};
use crate::graphql::execution_error::ExecutionError;
use crate::graphql::request_context::RequestContext;
use crate::graphql::{execution::system_context::SystemContext, validation::field::ValidatedField};

#[async_trait]
impl FieldResolver<Value> for FieldDefinition {
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        system_context: &'e SystemContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value, ExecutionError> {
        match field.name.as_str() {
            "name" => Ok(Value::String(self.name.node.as_str().to_owned())),
            "description" => Ok(self
                .description
                .clone()
                .map(|v| Value::String(v.node))
                .unwrap_or(Value::Null)),
            "type" => {
                self.ty
                    .resolve_value(&field.subfields, system_context, request_context)
                    .await
            }
            "args" => {
                self.arguments
                    .resolve_value(&field.subfields, system_context, request_context)
                    .await
            }
            "isDeprecated" => Ok(Value::Bool(false)), // TODO
            "deprecationReason" => Ok(Value::Null),   // TODO
            "__typename" => Ok(Value::String("__Field".to_string())),
            field_name => Err(ExecutionError::InvalidField(field_name.to_owned(), "Field")),
        }
    }
}
