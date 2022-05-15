use super::operations_context;
use crate::request_context::RequestContext;
use crate::validation::operation::ValidatedOperation;
use crate::OperationsPayload;
use crate::{
    error::ExecutionError, introspection::schema::Schema,
    validation::document_validator::DocumentValidator,
};
use async_graphql_parser::types::ExecutableDocument;
use async_graphql_parser::Pos;

use anyhow::Result;

use operations_context::{OperationsContext, QueryResponse};
use payas_deno::DenoExecutorPool;
use payas_model::model::{mapped_arena::SerializableSlab, system::ModelSystem, ContextType};
use payas_sql::DatabaseExecutor;
use serde_json::Value;
use tracing::{error, instrument};

/// Encapsulates the information required by the [crate::resolve] function.
///
/// A server implementation should call [crate::create_operations_executor] and
/// store the returned value, passing a reference to it each time it calls
/// `resolve`.
///
/// For example, in actix, this should be added to the server using `app_data`.
pub struct OperationsExecutor {
    pub(crate) database_executor: DatabaseExecutor,
    pub(crate) deno_execution_pool: DenoExecutorPool,
    pub(crate) system: ModelSystem,
    pub(crate) schema: Schema,
    pub allow_introspection: bool,
}

impl OperationsExecutor {
    pub async fn execute(
        &self,
        operations_payload: OperationsPayload,
        request_context: RequestContext,
    ) -> Result<Vec<(String, QueryResponse)>> {
        let request_context = create_mapped_context(&self.system.contexts, &request_context)?;

        self.execute_with_request_context(operations_payload, request_context)
            .await
    }

    // A version of execute that is suitable to be exposed through a shim to services
    #[instrument(
        name = "OperationsExecutor::execute_with_request_context"
        skip_all
        )]
    pub async fn execute_with_request_context(
        &self,
        operations_payload: OperationsPayload,
        request_context: Value,
    ) -> Result<Vec<(String, QueryResponse)>> {
        let (operation, query_context) =
            self.create_query_context(operations_payload, &request_context)?;

        query_context.resolve_operation(operation).await
    }

    #[instrument(skip(self, operations_payload, request_context))]
    fn create_query_context<'a>(
        &'a self,
        operations_payload: OperationsPayload,
        request_context: &'a serde_json::Value,
    ) -> Result<(ValidatedOperation, OperationsContext<'a>), ExecutionError> {
        let document = Self::parse_query(operations_payload.query)?;

        let document_validator = DocumentValidator::new(
            &self.schema,
            operations_payload.operation_name,
            operations_payload.variables,
        );

        document_validator.validate(document).map(|validated| {
            (
                validated,
                OperationsContext {
                    executor: self,
                    system: &self.system,
                    schema: &self.schema,
                    request_context,
                },
            )
        })
    }

    #[instrument(name = "OperationsExecutor::parse_query")]
    fn parse_query(query: String) -> Result<ExecutableDocument, ExecutionError> {
        async_graphql_parser::parse_query(query).map_err(|error| {
            error!(%error, "Failed to parse query");
            let (message, pos1, pos2) = match error {
                async_graphql_parser::Error::Syntax {
                    message,
                    start,
                    end,
                } => (format!("Syntax error {message}"), start, end),
                async_graphql_parser::Error::MultipleRoots { root, schema, pos } => {
                    (format!("Multiple roots of {root} type"), schema, Some(pos))
                }
                async_graphql_parser::Error::MissingQueryRoot { pos } => {
                    ("Missing query root".to_string(), pos, None)
                }
                async_graphql_parser::Error::MultipleOperations {
                    anonymous,
                    operation,
                } => (
                    "Multiple operations".to_string(),
                    anonymous,
                    Some(operation),
                ),
                async_graphql_parser::Error::OperationDuplicated {
                    operation: _,
                    first,
                    second,
                } => ("Operation duplicated".to_string(), first, Some(second)),
                async_graphql_parser::Error::FragmentDuplicated {
                    fragment,
                    first,
                    second,
                } => (
                    format!("Fragment {fragment} duplicated"),
                    first,
                    Some(second),
                ),
                async_graphql_parser::Error::MissingOperation => {
                    ("Missing operation".to_string(), Pos::default(), None)
                }
                _ => ("Unknown error".to_string(), Pos::default(), None),
            };

            ExecutionError::QueryParsingFailed(message, pos1, pos2)
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
