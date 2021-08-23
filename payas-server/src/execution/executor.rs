use super::query_context;
use crate::introspection::schema::Schema;
use async_graphql_parser::{parse_query, types::DocumentOperations};

use anyhow::Result;

use payas_model::{
    model::{mapped_arena::SerializableSlab, system::ModelSystem, ContextSource, ContextType},
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
            schema,
            system,
            database,
            request_context,
        },
    )
}

// TODO: Generalize to handle other context types and sources
fn create_request_contexts(
    contexts: &SerializableSlab<ContextType>,
    jwt_claims: Option<Value>,
) -> Value {
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
) -> Result<Vec<(String, QueryResponse)>> {
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

    operations
        .iter()
        .flat_map(|query| match query_context.resolve_operation(query) {
            Ok(resolved) => resolved.into_iter().map(Ok).collect(),
            Err(err) => vec![Err(err)],
        })
        .collect()
}
