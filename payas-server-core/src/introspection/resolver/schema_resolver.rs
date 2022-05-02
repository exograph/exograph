use crate::introspection::schema::{
    Schema, MUTATION_ROOT_TYPENAME, QUERY_ROOT_TYPENAME, SUBSCRIPTION_ROOT_TYPENAME,
};
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
        query_context: &'e OperationsContext<'e>,
        field: &ValidatedField,
    ) -> Result<Value> {
        let schema = query_context.schema;
        match field.name.as_str() {
            "types" => {
                self.type_definitions
                    .resolve_value(query_context, &field.subfields)
                    .await
            }
            "queryType" => {
                schema
                    .get_type_definition(QUERY_ROOT_TYPENAME)
                    .resolve_value(query_context, &field.subfields)
                    .await
            }
            "mutationType" => {
                schema
                    .get_type_definition(MUTATION_ROOT_TYPENAME)
                    .resolve_value(query_context, &field.subfields)
                    .await
            }
            "subscriptionType" => {
                schema
                    .get_type_definition(SUBSCRIPTION_ROOT_TYPENAME)
                    .resolve_value(query_context, &field.subfields)
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
