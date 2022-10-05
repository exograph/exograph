use async_trait::async_trait;
use futures::StreamExt;

use crate::plugin::SystemResolutionError;
use crate::system::SystemResolver;
use crate::validation::field::ValidatedField;
use crate::validation::operation::ValidatedOperation;
use crate::{request_context::RequestContext, QueryResponse};
use crate::{FieldResolver, QueryResponseBody};

/// Resolver for the root operation.
///
/// The operation may be a query or a mutation and may be for data or for introspection.
///
#[async_trait]
impl FieldResolver<QueryResponse, SystemResolutionError, SystemResolver> for ValidatedOperation {
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        system_resolver: &'e SystemResolver,
        request_context: &'e RequestContext<'e>,
    ) -> Result<QueryResponse, SystemResolutionError> {
        let stream = futures::stream::iter(system_resolver.subsystem_resolvers.iter()).then(
            |resolver| async {
                resolver
                    .resolve(
                        field,
                        self.typ,
                        request_context,
                        system_resolver.resolve_operation_fn(),
                    )
                    .await
            },
        );

        futures::pin_mut!(stream);

        // Really find_map(), but StreamExt::find_map() is not available.
        let QueryResponse { body, headers } = loop {
            match stream.next().await {
                Some(next_val) => {
                    // Found a resolver that could return a value (or an error).
                    if let Some(val) = next_val {
                        break val.map_err(|e| e.into());
                    }
                }
                None => {
                    // The steam has been exhausted, so return error.
                    break Err(SystemResolutionError::Generic(
                        "No suitable resolver found".to_string(),
                    ));
                }
            }
        }?;

        // A proceed call in an around interceptor may have returned more fields that necessary (just like a normal service),
        // so we need to filter out the fields that are not needed.
        // TODO: Validate that all requested fields are present in the response.
        let field_selected_response_body = match body {
            QueryResponseBody::Json(value @ serde_json::Value::Object(_)) => {
                let resolved_set = value
                    .resolve_fields(&field.subfields, &(), request_context)
                    .await?;
                QueryResponseBody::Json(serde_json::Value::Object(
                    resolved_set.into_iter().collect(),
                ))
            }
            _ => body,
        };

        Ok(QueryResponse {
            body: field_selected_response_body,
            headers,
        })
    }
}
