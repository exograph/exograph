use super::query_context;
use crate::request_context::RequestContext;
use crate::{
    error::ExecutionError,
    introspection::schema::Schema,
    validation::{document::ValidatedDocument, document_validator::DocumentValidator},
};
use async_graphql_parser::{parse_query, types::OperationType};

use anyhow::Result;

use futures::future::join_all;
use payas_deno::DenoExecutor;
use payas_model::model::{mapped_arena::SerializableSlab, system::ModelSystem, ContextType};
use payas_sql::DatabaseExecutor;
use query_context::{QueryContext, QueryResponse};
use serde_json::{Map, Value};

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
        let request_context = create_mapped_context(&self.system.contexts, &request_context)?;

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
        let (document, query_context) =
            self.create_query_context(operation_name, query_str, variables, &request_context)?;

        let resolutions = match document.operation_typ {
            OperationType::Query => {
                // process queries concurrently

                let query_resolution_futures: Vec<_> = document
                    .operations
                    .into_iter()
                    .map(|query| query_context.resolve_operation(query))
                    .collect();
                join_all(query_resolution_futures).await
            }
            OperationType::Mutation => {
                // process mutations sequentially
                let mut mutation_resolution = vec![];
                for mutation in document.operations.into_iter() {
                    let result = query_context.resolve_operation(mutation).await;
                    mutation_resolution.push(result);
                }
                mutation_resolution
            }
            OperationType::Subscription => todo!(),
        };

        resolutions
            .into_iter()
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
        variables: Option<&'a Map<String, Value>>,
        request_context: &'a serde_json::Value,
    ) -> Result<(ValidatedDocument, QueryContext<'a>), ExecutionError> {
        let document = parse_query(query_str).unwrap();

        let document_validator = DocumentValidator::new(self.schema, operation_name, variables);

        document_validator.validate(document).map(|validated| {
            (
                validated,
                QueryContext {
                    executor: self,
                    request_context,
                },
            )
        })
    }
}

fn create_mapped_context(
    contexts: &SerializableSlab<ContextType>,
    request_context: &RequestContext,
) -> Result<Value> {
    let mapped_contexts = contexts
        .iter()
        .map(|(_, context)| {
            Ok((
                context.name.clone(),
                extract_context(request_context, context)?,
            ))
        })
        .collect::<Result<_>>()?;

    Ok(Value::Object(mapped_contexts))
}

fn extract_context(request_context: &RequestContext, context: &ContextType) -> Result<Value> {
    Ok(Value::Object(
        context
            .fields
            .iter()
            .map(|field| {
                let field_value = request_context.extract_context_field_from_source(
                    &field.source.annotation_name,
                    &field.source.value,
                )?;
                Ok(field_value.map(|value| (field.name.clone(), value)))
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect(),
    ))
}
