use super::query_context;
use crate::request_context::RequestContext;
use crate::QueryPayload;
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
use serde_json::Value;

/// Opaque type encapsulating the information required by the [crate::resolve]
/// function.
///
/// A server implementation should call [crate::create_query_executor] and store the
/// returned value, passing a reference to it each time it calls `resolve`.
///
/// For example, in actix, this should be added to the server using `app_data`.
pub struct QueryExecutor {
    pub database_executor: DatabaseExecutor,
    pub deno_execution: DenoExecutor,
    pub system: ModelSystem,
    pub schema: Schema,
}

impl QueryExecutor {
    pub async fn execute(
        &self,
        query_payload: QueryPayload,
        request_context: RequestContext,
    ) -> Result<Vec<(String, QueryResponse)>> {
        let request_context = create_mapped_context(&self.system.contexts, &request_context)?;

        self.execute_with_request_context(query_payload, request_context)
            .await
    }

    // A version of execute that is suitable to be exposed through a shim to services
    pub async fn execute_with_request_context(
        &self,
        query_payload: QueryPayload,
        request_context: Value,
    ) -> Result<Vec<(String, QueryResponse)>> {
        let (document, query_context) =
            self.create_query_context(query_payload, &request_context)?;

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

    fn create_query_context<'a>(
        &'a self,
        query_payload: QueryPayload,
        request_context: &'a serde_json::Value,
    ) -> Result<(ValidatedDocument, QueryContext<'a>), ExecutionError> {
        let document = parse_query(query_payload.query).unwrap();

        let document_validator = DocumentValidator::new(
            &self.schema,
            query_payload.operation_name,
            query_payload.variables,
        );

        document_validator.validate(document).map(|validated| {
            (
                validated,
                QueryContext {
                    executor: self,
                    system: &self.system,
                    schema: &self.schema,
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
