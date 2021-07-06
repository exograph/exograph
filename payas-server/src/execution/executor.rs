use super::query_context;
use crate::{execution::resolver::GraphQLExecutionError, introspection::schema::Schema};
use async_graphql_parser::{parse_query, types::DocumentOperations};

use anyhow::Result;
use id_arena::Arena;
use payas_model::{
    model::{system::ModelSystem, ContextSource, ContextType},
    sql::database::Database,
};
use query_context::*;
use serde_json::{Map, Value};

pub fn create_query_context<'a>(
    system: &'a ModelSystem,
    schema: &'a Schema,
    database: &'a Database,
    operation_name: Option<&'a str>,
    query_str: &'a str,
    variables: &'a Option<&'a Map<String, Value>>,
    request_context: &'a serde_json::Value,
) -> (DocumentOperations, QueryContext<'a>) {
    let document = parse_query(query_str).unwrap();

    (
        document.operations,
        QueryContext {
            operation_name,
            fragment_definitions: document.fragments,
            variables,
            schema: &schema,
            system,
            database,
            request_context,
        },
    )
}

// TODO: Generalize to handle other context types and sources
fn create_request_contexts(contexts: &Arena<ContextType>, jwt_claims: Option<Value>) -> Value {
    let mapped_contexts = contexts
        .iter()
        .flat_map(|(_, context)| {
            create_request_context(context, jwt_claims.clone())
                .map(|value| (context.name.clone(), value))
        })
        .collect();

    Value::Object(mapped_contexts)
}

fn create_request_context(context: &ContextType, jwt_claims: Option<Value>) -> Option<Value> {
    jwt_claims.map(|jwt_claims| {
        let json_fields: Map<String, Value> = context
            .fields
            .iter()
            .map(|field| match &field.source {
                ContextSource::Jwt { claim } => {
                    (field.name.clone(), jwt_claims.get(claim).unwrap().clone())
                }
            })
            .collect();

        Value::Object(json_fields)
    })
}

pub fn execute<'a>(
    system: &'a ModelSystem,
    schema: &'a Schema,
    database: &'a Database,
    operation_name: Option<&'a str>,
    query_str: &'a str,
    variables: Option<&'a Map<String, Value>>,
    jwt_claims: Option<Value>,
) -> Result<String> {
    let request_context = create_request_contexts(&system.contexts, jwt_claims);

    let (operations, query_context) = create_query_context(
        system,
        schema,
        database,
        operation_name,
        query_str,
        &variables,
        &request_context,
    );

    let parts: Result<Vec<(String, QueryResponse)>, GraphQLExecutionError> = operations
        .iter()
        .flat_map(|query| match query_context.resolve_operation(query) {
            Ok(resolved) => resolved.into_iter().map(Ok).collect(),
            Err(err) => vec![Err(err)],
        })
        .collect();

    let parts = parts?;

    // TODO: More efficient (and ideally zero-copy) way to push the values to network
    let mut response = String::from(r#"{"data": {"#);
    parts.iter().enumerate().for_each(|(index, part)| {
        response.push('\"');
        response.push_str(part.0.as_str());
        response.push_str(r#"":"#);
        match &part.1 {
            QueryResponse::Json(value) => response.push_str(value.to_string().as_str()),
            QueryResponse::Raw(value) => response.push_str(value.as_str()),
        };
        if index != parts.len() - 1 {
            response.push_str(", ");
        }
    });
    response.push_str("}}");

    Ok(response)
}
