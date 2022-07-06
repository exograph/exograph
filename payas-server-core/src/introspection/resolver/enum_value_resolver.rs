use async_graphql_parser::types::EnumValueDefinition;
use async_trait::async_trait;
use serde_json::Value;

use crate::execution::resolver::{FieldResolver, GraphQLExecutionError};
use crate::request_context::RequestContext;
use crate::{execution::operations_context::OperationsContext, validation::field::ValidatedField};
use anyhow::{anyhow, Result};

#[async_trait]
impl FieldResolver<Value> for EnumValueDefinition {
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        _query_context: &'e OperationsContext<'e>,
        _request_context: &'e RequestContext<'e>,
    ) -> Result<Value> {
        match field.name.as_str() {
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
