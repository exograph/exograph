use async_graphql_value::indexmap::IndexMap;
use payas_deno::Arg;
use payas_deno_model::interceptor::Interceptor;
use payas_deno_model::model::ModelServiceSystem;
use payas_resolver_core::{
    request_context::RequestContext, validation::field::ValidatedField, ResolveOperationFn,
};
use serde_json::Value;

use crate::clay_execution::ClayCallbackProcessor;

use super::clay_execution::FnClaytipExecuteQuery;

use super::{
    clay_execution::{ClaytipMethodResponse, FnClaytipInterceptorProceed},
    deno_operation::construct_arg_sequence,
    ClayDenoExecutorPool, DenoExecutionError, InterceptedOperationInfo,
};

// For now allow too many arguments (we need to clean this to be able to work with DenoSystemContext, anyway)
#[allow(clippy::too_many_arguments)]
pub async fn execute_interceptor<'a>(
    interceptor: &'a Interceptor,
    system: &'a ModelServiceSystem,
    deno_execution_pool: &'a ClayDenoExecutorPool,
    request_context: &'a RequestContext<'a>,
    claytip_execute_query: &'a FnClaytipExecuteQuery<'a>,
    operation_name: String,
    operation_query: &'a ValidatedField,
    claytip_proceed_operation: Option<&'a FnClaytipInterceptorProceed<'a>>,
    resolve_operation: ResolveOperationFn<'a>,
) -> Result<(Value, Option<ClaytipMethodResponse>), DenoExecutionError> {
    let script = &system.scripts[interceptor.script];

    let arg_sequence: Vec<Arg> = construct_arg_sequence(
        &IndexMap::new(),
        &interceptor.arguments,
        system,
        &resolve_operation,
        request_context,
    )
    .await?;

    let callback_processor = ClayCallbackProcessor {
        claytip_execute_query,
        claytip_proceed: claytip_proceed_operation,
    };

    deno_execution_pool
        .execute_and_get_r(
            &script.path,
            &script.script,
            &interceptor.name,
            arg_sequence,
            Some(InterceptedOperationInfo {
                name: operation_name,
                query: serde_json::to_value(operation_query).unwrap(),
            }),
            callback_processor,
        )
        .await
        .map_err(DenoExecutionError::Deno)
}
