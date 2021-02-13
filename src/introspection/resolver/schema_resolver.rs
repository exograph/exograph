use graphql_parser::query::*;
use serde_json::Value;

use crate::introspection::{query_context, schema};

use super::resolver::*;
use query_context::QueryContext;
use schema::Schema;

impl<'a> FieldResolver for Schema<'a> {
    fn resolve_field(&self, query_context: &QueryContext<'_>, field: &Field<'_, String>) -> Value {
        match field.name.as_str() {
            "types" => self
                .type_definitions
                .resolve_value(query_context, &field.selection_set),
            "queryType" => query_context
                .schema
                .get_type_definition("Query")
                .resolve_value(query_context, &field.selection_set),
            "mutationType" => query_context
                .schema
                .get_type_definition("Mutation")
                .resolve_value(query_context, &field.selection_set),
            "subscriptionType" => query_context
                .schema
                .get_type_definition("Subscription")
                .resolve_value(query_context, &field.selection_set),
            "directives" => Value::Null, // TODO
            field_name => todo!("Invalid field {:?} for Schema", field_name), // TODO: Make it a proper error
        }
    }
}
