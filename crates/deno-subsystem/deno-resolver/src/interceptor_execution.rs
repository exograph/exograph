use async_graphql_value::indexmap::IndexMap;
use core_resolver::{
    request_context::RequestContext,
    system_resolver::{ClaytipExecuteQueryFn, SystemResolver},
    validation::field::ValidatedField,
};
use deno_model::interceptor::Interceptor;
use payas_deno::Arg;
use serde_json::Value;

use crate::{deno_operation::construct_arg_sequence, plugin::DenoSubsystemResolver};

use super::{
    clay_execution::{ClayCallbackProcessor, ClaytipMethodResponse, FnClaytipInterceptorProceed},
    claytip_ops::InterceptedOperationInfo,
    deno_execution_error::DenoExecutionError,
};

pub async fn execute_interceptor<'a>(
    interceptor: &'a Interceptor,
    subsystem_resolver: &'a DenoSubsystemResolver,
    request_context: &'a RequestContext<'a>,
    claytip_execute_query: &'a ClaytipExecuteQueryFn<'a>,
    operation: &'a ValidatedField,
    claytip_proceed_operation: Option<&'a FnClaytipInterceptorProceed<'a>>,
    system_resolver: &'a SystemResolver,
) -> Result<(Value, Option<ClaytipMethodResponse>), DenoExecutionError> {
    let script = &subsystem_resolver.subsystem.scripts[interceptor.script];

    let arg_sequence: Vec<Arg> = construct_arg_sequence(
        &IndexMap::new(),
        &interceptor.arguments,
        &subsystem_resolver.subsystem,
        system_resolver,
        request_context,
    )
    .await?;

    let callback_processor = ClayCallbackProcessor {
        claytip_execute_query,
        claytip_proceed: claytip_proceed_operation,
    };

    subsystem_resolver
        .executor
        .execute_and_get_r(
            &script.path,
            &script.script,
            &interceptor.name,
            arg_sequence,
            Some(InterceptedOperationInfo {
                name: operation.name.to_string(),
                query: serde_json::to_value(operation).unwrap(),
            }),
            callback_processor,
        )
        .await
        .map_err(DenoExecutionError::Deno)
}
