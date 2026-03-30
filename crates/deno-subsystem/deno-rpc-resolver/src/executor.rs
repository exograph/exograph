// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Deno-specific implementation of `ModuleRpcExecutor`.

use async_trait::async_trait;

use common::context::RequestContext;
use core_resolver::exograph_execute_query;
use core_resolver::system_resolver::{ExographExecuteQueryFn, GraphQLSystemResolver};

use exo_deno::{Arg, deno_executor_pool::DenoScriptDefn, error::DenoError};
use subsystem_model_util::module::ModuleMethod;
use subsystem_model_util::subsystem::ModuleSubsystem;
use subsystem_rpc_resolver_util::executor::{
    ModuleArg, ModuleRpcExecutionError, ModuleRpcExecutor,
};
#[allow(unused_imports)] // used by exograph_execute_query! macro
use {
    common::operation_payload::OperationsPayload,
    core_resolver::{QueryResponse, QueryResponseBody},
    futures::FutureExt,
};

// Re-use the executor pool type and config from deno-core-resolver
pub use deno_core_resolver::ExoDenoExecutorPool;
use deno_core_resolver::exo_execution::{ExoCallbackProcessor, ExographMethodResponse};
use deno_core_resolver::exograph_ops::InterceptedOperationInfo;

/// Deno executor for RPC method calls.
pub struct DenoRpcExecutor {
    pub executor: std::sync::Arc<ExoDenoExecutorPool>,
}

#[async_trait]
impl ModuleRpcExecutor for DenoRpcExecutor {
    async fn execute<'a>(
        &'a self,
        method: &'a ModuleMethod,
        args: Vec<ModuleArg>,
        subsystem: &'a ModuleSubsystem,
        request_context: &'a RequestContext<'a>,
        system_resolver: &'a GraphQLSystemResolver,
    ) -> Result<(serde_json::Value, Vec<(String, String)>), ModuleRpcExecutionError> {
        let script = &subsystem.scripts[method.script];
        let deserialized: DenoScriptDefn = serde_json::from_slice(&script.script).map_err(|e| {
            ModuleRpcExecutionError::Internal(format!("Script deserialization: {e}"))
        })?;

        // Convert ModuleArg to exo-deno Arg
        let deno_args: Vec<Arg> = args
            .into_iter()
            .map(|arg| match arg {
                ModuleArg::Value(v) => Arg::Serde(v),
                ModuleArg::Shim(name) => Arg::Shim(name),
            })
            .collect();

        // Use the same exograph_execute_query! macro as the GraphQL resolver
        let exograph_execute_query: &ExographExecuteQueryFn =
            exograph_execute_query!(system_resolver, request_context);

        let callback_processor = ExoCallbackProcessor {
            exograph_execute_query,
            exograph_proceed: None,
        };

        let call_context: Option<InterceptedOperationInfo> = None;

        let (result, response) = self
            .executor
            .execute_and_get_r(
                &script.path,
                deserialized,
                &method.name,
                deno_args,
                call_context,
                callback_processor,
            )
            .await
            .map_err(|e| match e {
                DenoError::Explicit(msg) => ModuleRpcExecutionError::UserDisplayError(msg),
                other => ModuleRpcExecutionError::Internal(other.to_string()),
            })?;

        let headers = response
            .map(|r: ExographMethodResponse| r.headers)
            .unwrap_or_default();

        Ok((result, headers))
    }
}
