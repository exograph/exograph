use async_trait::async_trait;
use payas_model::model::operation::Interceptors;

use crate::{
    data::operation_mapper::OperationResolverResult,
    deno::interception::InterceptedOperation,
    execution::{
        resolver::FieldResolver,
        system_context::{QueryResponse, QueryResponseBody},
    },
    execution_error::ExecutionError,
    request_context::RequestContext,
    validation::field::ValidatedField,
    SystemContext,
};

#[async_trait]
pub trait OperationResolver<'a> {
    async fn resolve_operation(
        &'a self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<OperationResolverResult<'a>, ExecutionError>;

    async fn execute(
        &'a self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<QueryResponse, ExecutionError> {
        let resolve = move |field: &'a ValidatedField,
                            system_context: &'a SystemContext,
                            request_context: &'a RequestContext<'a>| {
            self.resolve_operation(field, system_context, request_context)
        };

        let intercepted_operation =
            InterceptedOperation::new(self.name(), self.interceptors().ordered());
        let QueryResponse { body, headers } = intercepted_operation
            .execute(field, system_context, request_context, &resolve)
            .await?;

        // A proceed call in an around interceptor may have returned more fields that necessary (just like a normal service),
        // so we need to filter out the fields that are not needed.
        // TODO: Validate that all requested fields are present in the response.
        let field_selected_response_body = match body {
            QueryResponseBody::Json(value @ serde_json::Value::Object(_)) => {
                let resolved_set = value
                    .resolve_fields(&field.subfields, system_context, request_context)
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

    fn name(&self) -> &str;

    fn interceptors(&self) -> &Interceptors;
}
