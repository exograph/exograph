use async_trait::async_trait;
use futures::FutureExt;
use payas_service_model::interceptor::Interceptor;
use payas_service_model::operation::Interceptors;
use serde_json::Value;

use payas_resolver_core::validation::field::ValidatedField;
use payas_resolver_core::{request_context::RequestContext, QueryResponse, QueryResponseBody};
use payas_resolver_deno::{DenoExecutionError, DenoSystemContext};

use crate::graphql::{
    data::data_operation::DataOperation, data::interception::InterceptedOperation,
    execution::field_resolver::FieldResolver, execution::system_context::SystemContext,
    execution_error::ExecutionError,
};

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
pub trait DatabaseOperationResolver<'a>: payas_database_model::operation::GraphQLOperation {
    async fn resolve_operation(
        &'a self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<DataOperation<'a>, ExecutionError>;

    async fn execute(
        &'a self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<QueryResponse, ExecutionError> {
        let resolve = move |field: &'a ValidatedField, request_context: &'a RequestContext<'a>| {
            async move {
                let data_operation = self
                    .resolve_operation(field, system_context, request_context)
                    .await
                    .map_err(|e| DenoExecutionError::Delegate(Box::new(e)))?;

                data_operation
                    .execute(system_context, request_context)
                    .await
                    .map_err(|e| DenoExecutionError::Delegate(Box::new(e)))
            }
            .boxed()
        };

        let intercepted_operation = InterceptedOperation::new(
            self.name(),
            compute_interceptors(self.name(), self.is_query(), system_context),
        );

        let resolve_operation_fn = system_context.resolve_operation_fn();

        let deno_system_context = DenoSystemContext {
            system: &system_context.system.service_subsystem,
            deno_execution_pool: &system_context.deno_execution_pool,
            resolve_operation_fn,
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
}

// TODO: Fix this duplication. Once the final plugin refactoring is done, this will look very different.
#[async_trait]
pub trait ServiceOperationResolver<'a>: payas_service_model::operation::GraphQLOperation {
    async fn resolve_operation(
        &'a self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<DataOperation<'a>, ExecutionError>;

    async fn execute(
        &'a self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<QueryResponse, ExecutionError> {
        let resolve = move |field: &'a ValidatedField, request_context: &'a RequestContext<'a>| {
            async move {
                let data_operation = self
                    .resolve_operation(field, system_context, request_context)
                    .await
                    .map_err(|e| DenoExecutionError::Delegate(Box::new(e)))?;

                data_operation
                    .execute(system_context, request_context)
                    .await
                    .map_err(|e| DenoExecutionError::Delegate(Box::new(e)))
            }
            .boxed()
        };

        let intercepted_operation = InterceptedOperation::new(
            self.name(),
            compute_interceptors(self.name(), self.is_query(), system_context),
        );

        let resolve_operation_fn = system_context.resolve_operation_fn();

        let deno_system_context = DenoSystemContext {
            system: &system_context.system.service_subsystem,
            deno_execution_pool: &system_context.deno_execution_pool,
            resolve_operation_fn,
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
}

fn compute_interceptors<'a>(
    operation_name: &str,
    is_query: bool,
    system_context: &'a SystemContext,
) -> Vec<&'a Interceptor> {
    let all_interceptors = &system_context.system.service_subsystem.interceptors;

    let interceptors_map = if is_query {
        &system_context.system.query_interceptors
    } else {
        &system_context.system.mutation_interceptors
    };

    let interceptors = interceptors_map
        .get(operation_name)
        .map(|interceptor_indices| {
            interceptor_indices
                .iter()
                .map(|index| &all_interceptors[*index])
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Interceptors { interceptors }.ordered()
}
