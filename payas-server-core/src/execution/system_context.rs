use crate::execution_error::ExecutionError;
use crate::request_context::RequestContext;
use crate::validation::{document_validator::DocumentValidator, operation::ValidatedOperation};
use crate::validation_error::ValidationError;
use crate::OperationsPayload;
use crate::{introspection::definition::root_element::RootElement, introspection::schema::Schema};
use async_graphql_parser::types::ExecutableDocument;
use async_graphql_parser::Pos;

use crate::deno_integration::ClayDenoExecutorPool;
use payas_model::model::system::ModelSystem;
use payas_sql::DatabaseExecutor;
use tracing::{error, instrument};

use async_trait::async_trait;

use super::query_response::{QueryResponse, QueryResponseBody};
use super::resolver::FieldResolver;

use crate::{data::data_resolver::DataResolver, validation::field::ValidatedField};

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
    #[instrument(
        name = "OperationsExecutor::execute"
        skip_all
        )]
    pub async fn execute<'e>(
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

/**
Go through the FieldResolver (thus through the generic support offered by Resolver) and
so that we can support fragments in top-level queries in such as:

```graphql
{
  ...query_info
}

fragment query_info on Query {
  __type(name: "Concert") {
    name
  }

  __schema {
      types {
      name
    }
  }
}
```
*/
#[async_trait]
impl FieldResolver<QueryResponse> for ValidatedOperation {
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        system_context: &'e SystemContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<QueryResponse, ExecutionError> {
        let name = field.name.as_str();

        if name.starts_with("__") {
            let introspection_root = RootElement {
                operation_type: &self.typ,
                name,
            };

            let body = introspection_root
                .resolve_field(field, system_context, request_context)
                .await?;

            Ok(QueryResponse {
                body: QueryResponseBody::Json(body),
                headers: vec![],
            })
        } else {
            system_context
                .system
                .resolve(field, &self.typ, system_context, request_context)
                .await
        }
    }
}
