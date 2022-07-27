use crate::graphql::execution_error::ExecutionError;
use crate::graphql::introspection::schema::{
    Schema, MUTATION_ROOT_TYPENAME, QUERY_ROOT_TYPENAME, SUBSCRIPTION_ROOT_TYPENAME,
};
use crate::graphql::request_context::RequestContext;
use crate::graphql::validation::field::ValidatedField;
use async_trait::async_trait;
use serde_json::Value;

use crate::graphql::execution::field_resolver::FieldResolver;
use crate::graphql::execution::system_context::SystemContext;

use super::resolver_support::Resolver;

#[async_trait]
impl FieldResolver<Value, ExecutionError, SystemContext> for Schema {
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        system_context: &'e SystemContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value, ExecutionError> {
        let schema = &system_context.schema;
        match field.name.as_str() {
            "types" => {
                self.type_definitions
                    .resolve_value(&field.subfields, system_context, request_context)
                    .await
            }
            "queryType" => {
                schema
                    .get_type_definition(QUERY_ROOT_TYPENAME)
                    .resolve_value(&field.subfields, system_context, request_context)
                    .await
            }
            "mutationType" => {
                schema
                    .get_type_definition(MUTATION_ROOT_TYPENAME)
                    .resolve_value(&field.subfields, system_context, request_context)
                    .await
            }
            "subscriptionType" => {
                schema
                    .get_type_definition(SUBSCRIPTION_ROOT_TYPENAME)
                    .resolve_value(&field.subfields, system_context, request_context)
                    .await
            }
            "directives" => Ok(Value::Array(vec![])), // TODO
            "description" => Ok(Value::String("Top-level schema".to_string())),
            "__typename" => Ok(Value::String("__Schema".to_string())),
            field_name => Err(ExecutionError::InvalidField(
                field_name.to_owned(),
                "Schema",
            )),
        }
    }
}
