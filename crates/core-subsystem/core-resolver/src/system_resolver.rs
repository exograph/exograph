use async_graphql_parser::{
    types::{ExecutableDocument, OperationType},
    Pos,
};
use core_plugin::interception::{
    InterceptionMap, InterceptionTree, InterceptorIndexWithSubsystemIndex,
};
use futures::{future::BoxFuture, StreamExt};
use maybe_owned::MaybeOwned;
use serde_json::Value;
use thiserror::Error;
use tracing::{error, instrument};

use crate::{
    introspection::definition::schema::Schema,
    plugin::{subsystem_resolver::SubsystemResolver, SubsystemResolutionError},
    request_context::RequestContext,
    validation::{
        document_validator::DocumentValidator, field::ValidatedField,
        operation::ValidatedOperation, validation_error::ValidationError,
    },
    FieldResolver, OperationsPayload, QueryResponse,
};

pub type ResolveOperationFn<'r> = Box<
    dyn Fn(
            OperationsPayload,
            MaybeOwned<'r, RequestContext<'r>>,
        ) -> BoxFuture<
            'r,
            Result<Vec<(String, QueryResponse)>, Box<dyn std::error::Error + Send + Sync>>,
        >
        + 'r
        + Send
        + Sync,
>;

pub type ClaytipExecuteQueryFn<'a> = dyn Fn(
        String,
        Option<serde_json::Map<String, Value>>,
        Value,
    ) -> BoxFuture<'a, Result<QueryResponse, SystemResolutionError>>
    + 'a
    + Send
    + Sync;

/// The top-level system resolver.
///
/// Delegates to subsystem resolvers to resolve individual operations.
pub struct SystemResolver {
    subsystem_resolvers: Vec<Box<dyn SubsystemResolver + Send + Sync>>,
    query_interception_map: InterceptionMap,
    mutation_interception_map: InterceptionMap,
    schema: Schema,
}

impl SystemResolver {
    pub fn new(
        subsystem_resolvers: Vec<Box<dyn SubsystemResolver + Send + Sync>>,
        query_interception_map: InterceptionMap,
        mutation_interception_map: InterceptionMap,
        schema: Schema,
    ) -> Self {
        Self {
            subsystem_resolvers,
            query_interception_map,
            mutation_interception_map,
            schema,
        }
    }

    /// Resolve the provided top-level operation (which may contain multiple queries, mutations, or subscription).
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
        name = "SystemResolver::resolve_root"
        skip_all
        )]
    pub async fn resolve_operations<'a>(
        &self,
        operations_payload: OperationsPayload,
        request_context: &RequestContext<'a>,
    ) -> Result<Vec<(String, QueryResponse)>, SystemResolutionError> {
        let operation = self.validate_operation(operations_payload)?;

        operation
            .resolve_fields(&operation.fields, self, request_context)
            .await
    }

    /// Resolve the provided top-level operation.
    ///
    /// # Returns
    /// A function that captures the SystemResolver (`self`) and returns a function that takes the
    /// operation and request context and returns a future that resolves the operation.
    ///
    /// # Implementation notes
    /// We use MaybeOwned<RequestContext> since in a few cases (see claytip_execute_query) we need
    /// to pass a newly created owned object and in most other cases we need to pass an existing
    /// reference.
    pub fn resolve_operation_fn<'r>(&'r self) -> ResolveOperationFn<'r> {
        Box::new(
            move |input: OperationsPayload, request_context: MaybeOwned<'r, RequestContext<'r>>| {
                Box::pin(async move {
                    self.resolve_operations(input, &request_context)
                        .await
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                })
            },
        )
    }

    /// Should we allow introspection queries?
    ///
    /// Implementation note: This works in conjunction with `SystemLoader`, which doesn't create the
    /// "introspection" subsystem if introspection is disabled.
    pub fn allow_introspection(&self) -> bool {
        self.subsystem_resolvers
            .iter()
            .any(|subsystem_resolver| subsystem_resolver.id() == "introspection")
    }

    /// Obtain the interception tree associated with the given operation
    pub fn applicable_interceptors(
        &self,
        operation_name: &str,
        operation_type: OperationType,
    ) -> Option<&InterceptionTree> {
        match operation_type {
            OperationType::Query => self.query_interception_map.get(operation_name),
            OperationType::Mutation => self.mutation_interception_map.get(operation_name),
            OperationType::Subscription => None,
        }
    }

    pub(super) async fn resolve_operation<'a>(
        &self,
        operation_type: OperationType,
        operation: &ValidatedField,
        request_context: &RequestContext<'a>,
    ) -> Result<QueryResponse, SystemResolutionError> {
        let stream =
            futures::stream::iter(self.subsystem_resolvers.iter()).then(|resolver| async {
                resolver
                    .resolve(operation, operation_type, request_context, self)
                    .await
            });

        futures::pin_mut!(stream);

        // Really a find_map(), but StreamExt::find_map() is not available
        while let Some(next_val) = stream.next().await {
            if let Some(val) = next_val {
                // Found a resolver that could return a value (or an error), so we are done resolving
                return val.map_err(|e| e.into());
            }
        }

        Err(SystemResolutionError::NoResolverFound)
    }

    pub(super) async fn invoke_interceptor<'a>(
        &self,
        interceptor: &InterceptorIndexWithSubsystemIndex,
        operation_type: OperationType,
        operation: &ValidatedField,
        proceeding_interception_tree: Option<&InterceptionTree>,
        request_context: &RequestContext<'a>,
    ) -> Result<Option<QueryResponse>, SystemResolutionError> {
        let interceptor_subsystem = &self.subsystem_resolvers[interceptor.subsystem_index];

        match proceeding_interception_tree {
            Some(proceeding_interception_tree) => interceptor_subsystem
                .invoke_proceeding_interceptor(
                    operation,
                    operation_type,
                    interceptor.interceptor_index,
                    proceeding_interception_tree,
                    request_context,
                    self,
                ),

            None => interceptor_subsystem.invoke_non_proceeding_interceptor(
                operation,
                operation_type,
                interceptor.interceptor_index,
                request_context,
                self,
            ),
        }
        .await
        .map_err(|e| e.into())
    }

    #[instrument(skip(self, operations_payload))]
    fn validate_operation(
        &self,
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
}

#[macro_export]
macro_rules! claytip_execute_query {
    ($resolve_query_fn:expr, $request_context:expr) => {
        &move |query_string: String,
               variables: Option<serde_json::Map<String, serde_json::Value>>,
               context_override: serde_json::Value| {
            use core_resolver::system_resolver::SystemResolutionError;
            use core_resolver::QueryResponseBody;
            use futures::FutureExt;
            use maybe_owned::MaybeOwned;

            let new_request_context = $request_context.with_override(context_override);
            async move {
                // execute query
                let result = $resolve_query_fn(
                    core_resolver::OperationsPayload {
                        operation_name: None,
                        query: query_string,
                        variables,
                    },
                    MaybeOwned::Owned(new_request_context),
                )
                .await
                .map_err(SystemResolutionError::Delegate)?;

                // collate result into a single QueryResponse

                // since query execution results in a Vec<(String, QueryResponse)>, we want to
                // extract and collect all HTTP headers generated in QueryResponses
                let headers = result
                    .iter()
                    .flat_map(|(_, response)| response.headers.clone())
                    .collect::<Vec<_>>();

                // generate the body
                let body = result
                    .into_iter()
                    .map(|(name, response)| (name, response.body.to_json().unwrap()))
                    .collect::<serde_json::Map<_, _>>();

                Ok(QueryResponse {
                    body: QueryResponseBody::Json(serde_json::Value::Object(body)),
                    headers,
                })
            }
            .boxed()
        }
    };
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
            } => {
                // Error::Syntax's message is formatted with newlines, escape them properly
                let message = message.escape_debug();
                (format!("Syntax error:\\n{message}"), start, end)
            }
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

#[derive(Error, Debug)]
pub enum SystemResolutionError {
    #[error("{0}")]
    Delegate(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("{0}")]
    Validation(#[from] ValidationError),

    #[error("No subsystem resolver found")]
    NoResolverFound,

    #[error("{0}")]
    SubsystemResolutionError(#[from] SubsystemResolutionError),

    #[error("Subsystem error: {0}")]
    Generic(String),

    #[error("Around interceptor returned no response")]
    AroundInterceptorReturnedNoResponse,
}

impl SystemResolutionError {
    // Message that should be emitted when the error is returned to the user.
    // This should hide any internal details of the error.
    // TODO: Log the details of the error.
    pub fn user_error_message(&self) -> String {
        self.explicit_message()
            .unwrap_or_else(|| "Internal server error".to_string())
    }

    pub fn explicit_message(&self) -> Option<String> {
        match self {
            SystemResolutionError::Validation(error) => Some(error.to_string()),
            SystemResolutionError::SubsystemResolutionError(error) => error.user_error_message(),
            SystemResolutionError::Delegate(error) => error
                .downcast_ref::<SystemResolutionError>()
                .map(|error| error.user_error_message()),
            _ => None,
        }
    }
}
