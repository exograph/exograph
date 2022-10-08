use async_trait::async_trait;

use crate::interception::InterceptedOperation;
use crate::system_resolver::{SystemResolutionError, SystemResolver};
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
        let intercepted_operation = InterceptedOperation::new(
            self.typ,
            field,
            system_resolver.applicable_interceptors(&field.name, self.typ),
            system_resolver,
        );

        let QueryResponse { body, headers } =
            intercepted_operation.resolve(request_context).await?;

        // A proceed call in an around interceptor or a service call may have returned more fields
        // that necessary, so we need to filter out the fields that are not needed.
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
