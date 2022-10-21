use async_trait::async_trait;
use core_plugin::interception::InterceptionTree;
use serde_json::Value;

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
            Some(
                system_resolver
                    .applicable_interception_tree(&field.name, self.typ)
                    .unwrap_or(&InterceptionTree::Operation),
            ),
            self.typ,
            field,
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
