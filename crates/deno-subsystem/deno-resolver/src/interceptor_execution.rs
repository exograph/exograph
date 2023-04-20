// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_graphql_value::indexmap::IndexMap;
use core_plugin_interface::core_resolver::{
    request_context::RequestContext, system_resolver::ExographExecuteQueryFn, InterceptedOperation,
};
use deno_model::interceptor::Interceptor;
use exo_deno::Arg;
use serde_json::Value;

use crate::{deno_operation::construct_arg_sequence, plugin::DenoSubsystemResolver};

use super::{
    deno_execution_error::DenoExecutionError,
    exo_execution::{ExoCallbackProcessor, ExographMethodResponse},
    exograph_ops::InterceptedOperationInfo,
};

pub async fn execute_interceptor<'a>(
    interceptor: &Interceptor,
    subsystem_resolver: &'a DenoSubsystemResolver,
    request_context: &'a RequestContext<'a>,
    exograph_execute_query: &'a ExographExecuteQueryFn<'a>,
    intercepted_operation: &'a InterceptedOperation<'a>,
) -> Result<(Value, Option<ExographMethodResponse>), DenoExecutionError> {
    let script = &subsystem_resolver.subsystem.scripts[interceptor.script];

    let arg_sequence: Vec<Arg> = construct_arg_sequence(
        &IndexMap::new(),
        &interceptor.arguments,
        &subsystem_resolver.subsystem,
        request_context,
    )
    .await?;

    let intercepted_operation_resolver = || intercepted_operation.resolve(request_context);

    let callback_processor = ExoCallbackProcessor {
        exograph_execute_query,
        exograph_proceed: Some(&intercepted_operation_resolver),
    };

    subsystem_resolver
        .executor
        .execute_and_get_r(
            &script.path,
            &script.script,
            &interceptor.method_name,
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
