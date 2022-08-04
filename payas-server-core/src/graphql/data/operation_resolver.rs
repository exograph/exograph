use async_trait::async_trait;
use futures::FutureExt;
use payas_model::model::operation::Interceptors;
use serde_json::Value;

use payas_resolver_core::validation::field::ValidatedField;
use payas_resolver_core::{request_context::RequestContext, QueryResponse, QueryResponseBody};

use crate::graphql::{
    data::deno::interception::InterceptedOperation,
    data::operation_mapper::OperationResolverResult, execution::field_resolver::FieldResolver,
    execution::system_context::SystemContext, execution_error::ExecutionError,
};

use super::deno::deno_system_context::DenoSystemContext;
use super::deno::DenoExecutionError;

#[async_trait]
impl FieldResolver<Value, ExecutionError, SystemContext> for Value {
    async fn resolve_field<'a>(
        &'a self,
        field: &ValidatedField,
        _system_context: &'a SystemContext,
        _request_context: &'a RequestContext<'a>,
    ) -> Result<Value, ExecutionError> {
        let field_name = field.name.as_str();

        if let Value::Object(map) = self {
            map.get(field_name).cloned().ok_or_else(|| {
                ExecutionError::Generic(format!("No field named {} in Object", field_name))
            })
        } else {
            Err(ExecutionError::Generic(format!(
                "{} is not an Object and doesn't have any fields",
                field_name
            )))
        }
    }
}

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
        let resolve = move |field: &'a ValidatedField, request_context: &'a RequestContext<'a>| {
            async move {
                let resolver_result = self
                    .resolve_operation(field, system_context, request_context)
                    .await
                    .map_err(|e| DenoExecutionError::Delegate(Box::new(e)))?;

                resolver_result
                    .execute(field, system_context, request_context)
                    .await
                    .map_err(|e| DenoExecutionError::Delegate(Box::new(e)))
            }
            .boxed()
        };

        let intercepted_operation =
            InterceptedOperation::new(self.name(), self.interceptors().ordered());
        let resolve_query_owned_fn = system_context.curried_resolve_owned();
        let resolve_query_owned_fn = resolve_query_owned_fn.as_ref();

        let resolve_query_fn = system_context.curried_resolve();

        let deno_system_context = DenoSystemContext {
            system: &system_context.system,
            deno_execution_pool: &system_context.deno_execution_pool,
            resolve_query_fn: &resolve_query_fn,
            resolve_query_owned_fn,
        };

        let QueryResponse { body, headers } = intercepted_operation
            .execute(
                field,
                system_context,
                &deno_system_context,
                request_context,
                &resolve,
            )
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
