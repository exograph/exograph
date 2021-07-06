use crate::introspection::schema::Schema;
use async_graphql_parser::{types::Field, Positioned};
use serde_json::Value;

use crate::execution::query_context::QueryContext;
use crate::execution::resolver::*;

impl FieldResolver<Value> for Schema {
    fn resolve_field(
        &self,
        query_context: &QueryContext<'_>,
        field: &Positioned<Field>,
    ) -> Result<Value, GraphQLExecutionError> {
        match field.node.name.node.as_str() {
            "types" => self
                .type_definitions
                .resolve_value(query_context, &field.node.selection_set),
            "queryType" => query_context
                .schema
                .get_type_definition("Query")
                .resolve_value(query_context, &field.node.selection_set),
            "mutationType" => query_context
                .schema
                .get_type_definition("Mutation")
                .resolve_value(query_context, &field.node.selection_set),
            "subscriptionType" => query_context
                .schema
                .get_type_definition("Subscription")
                .resolve_value(query_context, &field.node.selection_set),
            "directives" => Ok(Value::Null), // TODO
            field_name => Err(GraphQLExecutionError::InvalidField(
                field_name.to_owned(),
                "Schema",
            )),
        }
    }
}
