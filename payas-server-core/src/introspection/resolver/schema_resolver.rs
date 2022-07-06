use crate::introspection::schema::{
    Schema, MUTATION_ROOT_TYPENAME, QUERY_ROOT_TYPENAME, SUBSCRIPTION_ROOT_TYPENAME,
};
use crate::request_context::RequestContext;
use crate::validation::field::ValidatedField;
use async_trait::async_trait;
use serde_json::Value;

use crate::execution::operations_context::OperationsContext;
use crate::execution::resolver::{FieldResolver, GraphQLExecutionError, Resolver};
use anyhow::{anyhow, Result};

#[async_trait]
impl FieldResolver<Value> for Schema {
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        operations_context: &'e OperationsContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value> {
        let schema = &operations_context.schema;
        match field.name.as_str() {
            "types" => {
                self.type_definitions
                    .resolve_value(&field.subfields, operations_context, request_context)
                    .await
            }
            "queryType" => {
                schema
                    .get_type_definition(QUERY_ROOT_TYPENAME)
                    .resolve_value(&field.subfields, operations_context, request_context)
                    .await
            }
            "mutationType" => {
                schema
                    .get_type_definition(MUTATION_ROOT_TYPENAME)
                    .resolve_value(&field.subfields, operations_context, request_context)
                    .await
            }
            "subscriptionType" => {
                schema
                    .get_type_definition(SUBSCRIPTION_ROOT_TYPENAME)
                    .resolve_value(&field.subfields, operations_context, request_context)
                    .await
            }
            "directives" => Ok(Value::Array(vec![])), // TODO
            "description" => Ok(Value::String("Top-level schema".to_string())),
            "__typename" => Ok(Value::String("__Schema".to_string())),
            field_name => Err(anyhow!(GraphQLExecutionError::InvalidField(
                field_name.to_owned(),
                "Schema",
            ))),
        }
    }
}
