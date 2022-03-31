use async_graphql_parser::types::Directive;
use async_trait::async_trait;
use serde_json::Value;

use crate::execution::resolver::{FieldResolver, GraphQLExecutionError};
use crate::{execution::query_context::QueryContext, validation::field::ValidatedField};
use anyhow::{anyhow, Result};

#[async_trait(?Send)]
impl FieldResolver<Value> for Directive {
    async fn resolve_field<'e>(
        &'e self,
        _query_context: &'e QueryContext<'e>,
        field: &ValidatedField,
    ) -> Result<Value> {
        match field.name.as_str() {
            "name" => Ok(Value::String(self.name.node.as_str().to_owned())),
            "description" => Ok(Value::Null),
            "isRepeatable" => Ok(Value::Bool(false)), // TODO
            "locations" => Ok(Value::Array(vec![])),  // TODO
            "args" => Ok(Value::Array(vec![])),       // TODO
            "__typename" => Ok(Value::String("__Directive".to_string())),
            field_name => Err(anyhow!(GraphQLExecutionError::InvalidField(
                field_name.to_owned(),
                "Directive"
            ))),
        }
    }
}
