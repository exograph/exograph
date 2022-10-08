use async_graphql_parser::{
    types::{ExecutableDocument, OperationType},
    Pos,
};
use async_trait::async_trait;
use futures::future::BoxFuture;
use maybe_owned::MaybeOwned;
use payas_core_model::serializable_system::{InterceptionMap, InterceptionTree};
use serde_json::Value;
use tracing::{error, instrument};

use crate::{
    introspection::definition::schema::Schema,
    plugin::{SubsystemResolver, SystemResolutionError},
    request_context::RequestContext,
    validation::{
        document_validator::DocumentValidator, field::ValidatedField,
        operation::ValidatedOperation, validation_error::ValidationError,
    },
    FieldResolver, OperationsPayload, QueryResponse, ResolveOperationFn,
};

pub struct SystemResolver {
    pub subsystem_resolvers: Vec<Box<dyn SubsystemResolver + Send + Sync>>,
    pub query_interception_map: InterceptionMap,
    pub mutation_interception_map: InterceptionMap,

    pub schema: Schema,
    pub allow_introspection: bool,
}

impl SystemResolver {
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
        name = "SystemResolver::resolve"
        skip_all
        )]
    pub async fn resolve<'e>(
        &'e self,
        operations_payload: OperationsPayload,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Vec<(String, QueryResponse)>, SystemResolutionError> {
        let operation = self.validate_operation(operations_payload)?;

        operation
            .resolve_fields(&operation.fields, self, request_context)
            .await
    }

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
                    self.resolve(input, &request_context)
                        .await
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                })
            },
        )
    }
}

pub type FnClaytipExecuteQuery<'a> = dyn Fn(
        String,
        Option<serde_json::Map<String, Value>>,
        Value,
    ) -> BoxFuture<'a, Result<QueryResponse, SystemResolutionError>>
    + 'a
    + Send
    + Sync;

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

#[async_trait]
impl FieldResolver<Value, SystemResolutionError, ()> for Value {
    async fn resolve_field<'a>(
        &'a self,
        field: &ValidatedField,
        _resolution_context: &'a (),
        _request_context: &'a RequestContext<'a>,
    ) -> Result<Value, SystemResolutionError> {
        let field_name = field.name.as_str();

        if let Value::Object(map) = self {
            map.get(field_name).cloned().ok_or_else(|| {
                SystemResolutionError::Generic(format!("No field named {} in Object", field_name))
            })
        } else {
            Err(SystemResolutionError::Generic(format!(
                "{} is not an Object and doesn't have any fields",
                field_name
            )))
        }
    }
}
