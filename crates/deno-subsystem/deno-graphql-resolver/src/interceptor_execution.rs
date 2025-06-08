// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use common::context::RequestContext;
use core_resolver::{
    InterceptedOperation, system_resolver::ExographExecuteQueryFn,
    validation::field::ValidatedField,
};
use deno_graphql_model::interceptor::Interceptor;
use exo_deno::{Arg, deno_executor_pool::DenoScriptDefn};
use indexmap::IndexMap;
use serde_json::Value;

use crate::{DenoSubsystemResolver, deno_operation::construct_arg_sequence};

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

    let deserialized: DenoScriptDefn = serde_json::from_slice(&script.script).unwrap();

    subsystem_resolver
        .executor
        .execute_and_get_r(
            &script.path,
            deserialized,
            &interceptor.method_name,
            arg_sequence,
            Some(InterceptedOperationInfo {
                name: intercepted_operation.operation().name.to_string(),
                query: operation_to_value(intercepted_operation.operation()),
            }),
            callback_processor,
        )
        .await
        .map_err(DenoExecutionError::Deno)
}

// We can't use Value::to_json, since the coversion from `Val` to `Value` doesn't map carries additional tags
// that don't work from the Deno side.
fn operation_to_value(operation: &ValidatedField) -> Value {
    let mut map = serde_json::Map::new();
    map.insert(
        "alias".to_string(),
        operation
            .alias
            .as_ref()
            .map(|alias| alias.to_string())
            .into(),
    );
    map.insert(
        "name".to_string(),
        Value::String(operation.name.to_string()),
    );
    map.insert(
        "arguments".to_string(),
        Value::Object(
            operation
                .arguments
                .iter()
                .map(|(key, value)| {
                    let json_value: serde_json::Value = value.clone().try_into().unwrap();
                    (key.to_string(), json_value)
                })
                .collect(),
        ),
    );
    map.insert(
        "subfields".to_string(),
        Value::Array(operation.subfields.iter().map(operation_to_value).collect()),
    );
    Value::Object(map)
}
