use super::query_context;
use crate::{introspection::schema::Schema, request_context::RequestContext};
use async_graphql_parser::{
    parse_query,
    types::{DocumentOperations, OperationType},
};

use anyhow::Result;

use futures::future::join_all;
use payas_deno::DenoExecutor;
use payas_model::model::{mapped_arena::SerializableSlab, system::ModelSystem, ContextType};
use payas_sql::DatabaseExecutor;
use query_context::{QueryContext, QueryResponse};
use serde_json::{Map, Value};
use typed_arena::Arena;

pub struct QueryExecutor<'a> {
    pub system: &'a ModelSystem,
    pub schema: &'a Schema,
    pub database_executor: &'a DatabaseExecutor<'a>,
    pub deno_execution: &'a DenoExecutor,
}

impl<'a> QueryExecutor<'a> {
    pub async fn execute(
        &'a self,
        operation_name: Option<&'a str>,
        query_str: &'a str,
        variables: Option<&'a Map<String, Value>>,
        request_context: RequestContext,
    ) -> Result<Vec<(String, QueryResponse)>> {
        let request_context = create_mapped_context(&self.system.contexts, &request_context);

        self.execute_with_request_context(operation_name, query_str, variables, request_context)
            .await
    }

    // A version of execute that is suitable to be exposed through a shim to services
    pub async fn execute_with_request_context(
        &'a self,
        operation_name: Option<&'a str>,
        query_str: &'a str,
        variables: Option<&'a Map<String, Value>>,
        request_context: Value,
    ) -> Result<Vec<(String, QueryResponse)>> {
        let (operations, query_context) =
            self.create_query_context(operation_name, query_str, &variables, &request_context);

        let mutation_operations = operations
            .iter()
            .filter(|(_, op)| op.node.ty == OperationType::Mutation);

        let query_operations = operations
            .iter()
            .filter(|(_, op)| op.node.ty == OperationType::Query);

        // process mutations one-by-one
        let mut mutation_resolution = vec![];
        for query in mutation_operations {
            let result = query_context.resolve_operation(query).await;
            mutation_resolution.push(result);
        }

        // process queries concurrently
        let query_resolution_futures: Vec<_> = query_operations
            .map(|query| query_context.resolve_operation(query))
            .collect();
        let query_resolution = join_all(query_resolution_futures).await;

        vec![]
            .into_iter()
            .chain(mutation_resolution.into_iter())
            .chain(query_resolution.into_iter())
            .flat_map(|query: Result<Vec<(String, QueryResponse)>>| match query {
                Ok(resolved) => resolved.into_iter().map(Ok).collect(),
                Err(err) => vec![Err(err)],
            })
            .collect()
    }

    fn create_query_context(
        &'a self,
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
                executor: self,
                request_context,
                field_arguments: Arena::new(),
            },
        )
    }
}

fn create_mapped_context(
    contexts: &SerializableSlab<ContextType>,
    request_context: &RequestContext,
) -> Value {
    let mapped_contexts = contexts
        .iter()
        .map(|(_, context)| {
            (
                context.name.clone(),
                request_context.to_json_context(context).unwrap(),
            )
        })
        .collect();

    Value::Object(mapped_contexts)
}
