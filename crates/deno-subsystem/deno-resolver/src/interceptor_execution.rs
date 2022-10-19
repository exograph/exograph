use async_graphql_value::indexmap::IndexMap;
use core_resolver::{
    request_context::RequestContext, system_resolver::ClaytipExecuteQueryFn, InterceptedOperation,
};
use deno_model::interceptor::Interceptor;
use payas_deno::Arg;
use serde_json::Value;

use crate::{deno_operation::construct_arg_sequence, plugin::DenoSubsystemResolver};

use super::{
    clay_execution::{ClayCallbackProcessor, ClaytipMethodResponse},
    claytip_ops::InterceptedOperationInfo,
    deno_execution_error::DenoExecutionError,
};

pub async fn execute_interceptor<'a>(
    interceptor: &Interceptor,
    subsystem_resolver: &'a DenoSubsystemResolver,
    request_context: &'a RequestContext<'a>,
    claytip_execute_query: &'a ClaytipExecuteQueryFn<'a>,
    intercepted_operation: &'a InterceptedOperation<'a>,
) -> Result<(Value, Option<ClaytipMethodResponse>), DenoExecutionError> {
    let script = &subsystem_resolver.subsystem.scripts[interceptor.script];

    let arg_sequence: Vec<Arg> = construct_arg_sequence(
        &IndexMap::new(),
        &interceptor.arguments,
        &subsystem_resolver.subsystem,
        request_context,
    )
    .await?;

    let intercepted_operation_resolver = || intercepted_operation.resolve(request_context);

    let callback_processor = ClayCallbackProcessor {
        claytip_execute_query,
        claytip_proceed: Some(&intercepted_operation_resolver),
    };

    subsystem_resolver
        .executor
        .execute_and_get_r(
            &script.path,
            &script.script,
            &interceptor.name,
            arg_sequence,
            Some(InterceptedOperationInfo {
                name: intercepted_operation.operation().name.to_string(),
                query: serde_json::to_value(intercepted_operation.operation()).unwrap(),
            }),
            callback_processor,
        )
        .await
        .map_err(DenoExecutionError::Deno)
}
