use super::query_context;
use crate::introspection::schema::Schema;
use crate::DataContext;
use async_graphql_parser::{parse_query, types::DocumentOperations};
use query_context::*;
use serde_json::{Map, Value};

pub fn create_query_context<'a>(
    data_context: &'a DataContext,
    schema: &'a Schema,
    operation_name: &'a str,
    query_str: &'a str,
    variables: &'a Option<&'a Map<String, Value>>,
) -> (DocumentOperations, QueryContext<'a>) {
    let document = parse_query(query_str).unwrap();

    (
        document.operations,
        QueryContext {
            operation_name,
            fragment_definitions: document.fragments,
            variables,
            schema: &schema,
            data_context,
        },
    )
}

pub fn execute<'a>(
    data_system: &'a DataContext,
    schema: &'a Schema,
    operation_name: &'a str,
    query_str: &'a str,
    variables: Option<&'a Map<String, Value>>,
) -> String {
    let (operations, query_context) =
        create_query_context(data_system, schema, operation_name, query_str, &variables);

    let parts: Vec<(String, QueryResponse)> = operations
        .iter()
        .flat_map(|query| query_context.resolve_operation(query))
        .collect();

    // TODO: More efficient (and ideally zero-copy) way to push the values to network
    let mut response = String::from("{\"data\": {");
    parts.iter().enumerate().for_each(|(index, part)| {
        response.push_str("\"");
        response.push_str(part.0.as_str());
        response.push_str("\":");
        match &part.1 {
            QueryResponse::Json(value) => response.push_str(value.to_string().as_str()),
            QueryResponse::Raw(value) => response.push_str(value.as_str()),
        };
        if index != parts.len() - 1 {
            response.push_str(", ");
        }
    });
    response.push_str("}}");

    response
}
