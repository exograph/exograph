// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_graphql_parser::{
    types::{ExecutableDocument, OperationType},
    Pos,
};
use core_plugin_shared::interception::{
    InterceptionMap, InterceptionTree, InterceptorIndexWithSubsystemIndex,
};
use futures::{future::BoxFuture, StreamExt};
use serde_json::Value;
use thiserror::Error;
use tokio::runtime::Handle;
use tracing::{error, instrument};

use crate::{
    introspection::definition::schema::Schema,
    plugin::{subsystem_resolver::SubsystemResolver, SubsystemResolutionError},
    request_context::RequestContext,
    validation::{
        document_validator::DocumentValidator, field::ValidatedField,
        operation::ValidatedOperation, validation_error::ValidationError,
    },
    FieldResolver, InterceptedOperation, OperationsPayload, QueryResponse,
};

pub type ExographExecuteQueryFn<'a> = dyn Fn(
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
    normal_query_depth_limit: usize,
    introspection_query_depth_limit: usize,
}

impl SystemResolver {
    pub fn new(
        subsystem_resolvers: Vec<Box<dyn SubsystemResolver + Send + Sync>>,
        query_interception_map: InterceptionMap,
        mutation_interception_map: InterceptionMap,
        schema: Schema,
        normal_query_depth_limit: usize,
        introspection_query_depth_limit: usize,
    ) -> Self {
        Self {
            subsystem_resolvers,
            query_interception_map,
            mutation_interception_map,
            schema,
            normal_query_depth_limit,
            introspection_query_depth_limit,
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
        name = "SystemResolver::resolve_operations"
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
    pub fn applicable_interception_tree(
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
                    .resolve_cdylib(
                        Handle::current(),
                        operation,
                        operation_type,
                        request_context,
                        self,
                    )
                    .await
            });

        futures::pin_mut!(stream);

        // Really a find_map(), but StreamExt::find_map() is not available
        while let Some(next_val) = stream.next().await {
            if let Some(val) = next_val? {
                // Found a resolver that could return a value (or an error), so we are done resolving
                return Ok(val);
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

        let intercepted_operation = InterceptedOperation::new(
            proceeding_interception_tree,
            operation_type,
            operation,
            self,
        );

        interceptor_subsystem
            .invoke_interceptor_cdylib(
                Handle::current(),
                interceptor.interceptor_index,
                &intercepted_operation,
                request_context,
                self,
            )
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
            self.normal_query_depth_limit,
            self.introspection_query_depth_limit,
        );

        document_validator.validate(document)
    }
}

#[macro_export]
macro_rules! exograph_execute_query {
    ($system_resolver:expr, $request_context:expr) => {
        &move |query_string: String,
               variables: Option<serde_json::Map<String, serde_json::Value>>,
               context_override: serde_json::Value| {
            use core_plugin_interface::core_resolver::{OperationsPayload, QueryResponseBody};
            use futures::FutureExt;

            let new_request_context = $request_context.with_override(context_override);
            async move {
                // execute query
                let result = $system_resolver
                    .resolve_operations(
                        OperationsPayload {
                            operation_name: None,
                            query: query_string,
                            variables,
                        },
                        &new_request_context,
                    )
                    .await?;

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

#[instrument(name = "system_resolver::parse_query")]
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

    #[error(
        "Attempt to resolve empty interceptor (proceed called from before/after interceptor?)"
    )]
    NoInterceptionTree,
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
