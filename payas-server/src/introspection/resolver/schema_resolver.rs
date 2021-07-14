use crate::introspection::schema::*;
use async_graphql_parser::{types::Field, Positioned};
use serde_json::Value;

use crate::execution::query_context::QueryContext;
use crate::execution::resolver::*;
use anyhow::{anyhow, Result};

impl FieldResolver<Value> for Schema {
    fn resolve_field(
        &self,
        query_context: &QueryContext<'_>,
        field: &Positioned<Field>,
    ) -> Result<Value> {
        match field.node.name.node.as_str() {
            "types" => self
                .type_definitions
                .resolve_value(query_context, &field.node.selection_set),
            "queryType" => query_context
                .schema
                .get_type_definition(QUERY_ROOT_TYPENAME)
                .resolve_value(query_context, &field.node.selection_set),
            "mutationType" => query_context
                .schema
                .get_type_definition(MUTATION_ROOT_TYPENAME)
                .resolve_value(query_context, &field.node.selection_set),
            "subscriptionType" => query_context
                .schema
                .get_type_definition(SUBSCRIPTION_ROOT_TYPENAME)
                .resolve_value(query_context, &field.node.selection_set),
            "directives" => Ok(Value::Null), // TODO
            "__typename" => Ok(Value::String("__Schema".to_string())),
            field_name => Err(anyhow!(GraphQLExecutionError::InvalidField(
                field_name.to_owned(),
                "Schema",
            ))),
        }
    }
}
