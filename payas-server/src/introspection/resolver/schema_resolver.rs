use crate::introspection::schema::*;
use async_graphql_parser::{types::Field, Positioned};
use async_trait::async_trait;
use serde_json::Value;

use crate::execution::query_context::QueryContext;
use crate::execution::resolver::*;
use anyhow::{anyhow, Result};

#[async_trait(?Send)]
impl FieldResolver<Value> for Schema {
    async fn resolve_field<'e>(
        &'e self,
        query_context: &'e QueryContext<'e>,
        field: &'e Positioned<Field>,
    ) -> Result<Value> {
        let schema = query_context.executor.schema;
        match field.node.name.node.as_str() {
            "types" => self
                .type_definitions
                .resolve_value(query_context, &field.node.selection_set).await,
            "queryType" => schema
                .get_type_definition(QUERY_ROOT_TYPENAME)
                .resolve_value(query_context, &field.node.selection_set).await,
            "mutationType" => schema
                .get_type_definition(MUTATION_ROOT_TYPENAME)
                .resolve_value(query_context, &field.node.selection_set).await,
            "subscriptionType" => schema
                .get_type_definition(SUBSCRIPTION_ROOT_TYPENAME)
                .resolve_value(query_context, &field.node.selection_set).await,
            "directives" => Ok(Value::Null), // TODO
            "__typename" => Ok(Value::String("__Schema".to_string())),
            field_name => Err(anyhow!(GraphQLExecutionError::InvalidField(
                field_name.to_owned(),
                "Schema",
            ))),
        }
    }
}
