use async_graphql_parser::types::ExecutableDocument;
use async_graphql_parser::Pos;
use maybe_owned::MaybeOwned;
use tracing::{error, instrument};

use payas_model::model::system::ModelSystem;
use payas_resolver_core::{
    request_context::RequestContext, OperationsPayload, QueryResponse, ResolveOperationFn,
};

use payas_sql::DatabaseExecutor;

use crate::graphql::{
    execution_error::ExecutionError,
    introspection::schema::Schema,
    validation::{
        document_validator::DocumentValidator, operation::ValidatedOperation,
        validation_error::ValidationError,
    },
};
use payas_resolver_deno::ClayDenoExecutorPool;

use super::field_resolver::FieldResolver;

/// Encapsulates the information required by the [crate::resolve] function.
///
/// A server implementation should call [crate::create_system_context] and
/// store the returned value, passing a reference to it each time it calls
/// `resolve`.
///
/// For example, in actix, this should be added to the server using `app_data`.
pub struct SystemContext {
    pub(crate) database_executor: DatabaseExecutor,
    pub(crate) deno_execution_pool: ClayDenoExecutorPool,
    pub(crate) system: ModelSystem,
    pub(crate) schema: Schema,
    pub allow_introspection: bool,
}

impl SystemContext {
    /// Resolve the provided top-level operation.
    ///
    /// Goes through the FieldResolver for ValidatedOperation (and thus get free support for `resolve_fields`)
    /// so that we can support fragments in top-level queries in such as:
    ///
    /// ```graphql
    /// {
    ///   ...query_info
    /// }
    ///
    /// fragment query_info on Query {
    ///   __type(name: "Concert") {
    ///     name
    ///   }
    ///
    ///   __schema {
    ///       types {
    ///       name
    ///     }
    ///   }
    /// }
    /// ```
    #[instrument(
        name = "OperationsExecutor::resolve"
        skip_all
        )]
    pub async fn resolve<'e>(
        &'e self,
        operations_payload: OperationsPayload,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Vec<(String, QueryResponse)>, ExecutionError> {
        let operation = self.validate_operation(operations_payload)?;

        operation
            .resolve_fields(&operation.fields, self, request_context)
            .await
    }

    #[instrument(skip(self, operations_payload))]
    fn validate_operation<'e>(
        &'e self,
        operations_payload: OperationsPayload,
    ) -> Result<ValidatedOperation, ValidationError> {
        let document = parse_query(operations_payload.query)?;

        let document_validator = DocumentValidator::new(
            &self.schema,
            operations_payload.operation_name,
            operations_payload.variables,
        );

        document_validator.validate(document)
    }

    /// Resolve the provided top-level operation.
    ///
    /// # Returns
    /// A function that captures the SystemContext and returns a function that takes the operation and request
    /// context and returns a future that resolves the operation.
    ///
    /// # Implementation notes
    /// We use MaybeOwned<RequestContext> since in a few cases (see claytip_execute_query) we need to pass a owned object and in
    /// most other cases we need to pass a reference.
    pub fn resolve_operation_fn<'r>(&'r self) -> ResolveOperationFn<'r> {
        Box::new(
            move |input: OperationsPayload, request_context: MaybeOwned<'r, RequestContext<'r>>| {
                Box::pin(async move {
                    self.resolve(input, &request_context)
                        .await
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                })
            },
        )
    }
}

#[instrument(name = "system_context::parse_query")]
fn parse_query(query: String) -> Result<ExecutableDocument, ValidationError> {
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

        ValidationError::QueryParsingFailed(message, pos1, pos2)
    })
}
