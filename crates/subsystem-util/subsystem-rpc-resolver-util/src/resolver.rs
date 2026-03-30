// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Generic RPC resolver for module subsystems (Deno, WASM).

use std::collections::HashMap;

use async_trait::async_trait;
use common::context::RequestContext;
use common::value::Val;
use core_resolver::access_solver::AccessSolverError;
use core_resolver::context_extractor::ContextExtractor;
use core_resolver::plugin::SubsystemRpcResolver;
use core_resolver::plugin::subsystem_rpc_resolver::{SubsystemRpcError, SubsystemRpcResponse};
use core_resolver::{QueryResponse, QueryResponseBody};
use http::StatusCode;
use rpc_introspection::RpcSchema;
use subsystem_model_util::subsystem::ModuleSubsystem;

use crate::executor::{ModuleArg, ModuleRpcExecutionError, ModuleRpcExecutor};
use crate::rpc_schema_builder::{self, RpcSchemaWithMapping};
use subsystem_resolver_util::access::check_module_access;

/// Generic RPC resolver parameterized by the executor (Deno, WASM, etc.).
pub struct ModuleSubsystemRpcResolver<E: ModuleRpcExecutor> {
    id: &'static str,
    subsystem: ModuleSubsystem,
    executor: E,
    rpc_schema: RpcSchema,
    /// Maps snake_case RPC method name → original camelCase operation name.
    method_name_map: HashMap<String, String>,
}

impl<E: ModuleRpcExecutor> ModuleSubsystemRpcResolver<E> {
    pub fn new(id: &'static str, subsystem: ModuleSubsystem, executor: E) -> Self {
        let RpcSchemaWithMapping {
            schema,
            method_name_map,
        } = rpc_schema_builder::build_rpc_schema(&subsystem);

        Self {
            id,
            subsystem,
            executor,
            rpc_schema: schema,
            method_name_map,
        }
    }
}

#[async_trait]
impl<E: ModuleRpcExecutor> SubsystemRpcResolver for ModuleSubsystemRpcResolver<E> {
    fn id(&self) -> &'static str {
        self.id
    }

    async fn resolve<'a>(
        &self,
        request_method: &str,
        request_params: &Option<serde_json::Value>,
        request_context: &'a RequestContext<'a>,
        system_resolver: &'a core_resolver::system_resolver::GraphQLSystemResolver,
    ) -> Result<Option<SubsystemRpcResponse>, SubsystemRpcError> {
        // Look up the method in our schema
        let rpc_method = match self.rpc_schema.method(request_method) {
            Some(m) => m,
            None => return Ok(None), // Not our method — let other subsystems try
        };

        // Validate parameters
        let validated_params = rpc_method
            .parse_params(request_params, &self.rpc_schema.components)
            .map_err(|e| SubsystemRpcError::InvalidParams(e.user_message()))?;

        // Map snake_case RPC name back to original operation name
        let original_name = self
            .method_name_map
            .get(request_method)
            .ok_or_else(|| SubsystemRpcError::MethodNotFound(request_method.to_string()))?;

        // Find the operation in queries or mutations
        let method_id = self
            .subsystem
            .queries
            .get_by_key(original_name)
            .and_then(|q| q.method_id)
            .or_else(|| {
                self.subsystem
                    .mutations
                    .get_by_key(original_name)
                    .and_then(|m| m.method_id)
            })
            .ok_or_else(|| SubsystemRpcError::MethodNotFound(request_method.to_string()))?;

        let method = &self.subsystem.methods[method_id];

        // Check access
        let access_allowed = check_module_access(method, &self.subsystem, request_context)
            .await
            .map_err(|e| match e {
                AccessSolverError::ContextExtraction(ce) => SubsystemRpcError::from(ce),
                _ => SubsystemRpcError::Authorization,
            })?;

        if !access_allowed {
            return Err(SubsystemRpcError::Authorization);
        }

        // Build argument sequence
        let args =
            build_arg_sequence(&validated_params, method, &self.subsystem, request_context).await?;

        // Execute via the subsystem-specific executor
        let (result, headers) = self
            .executor
            .execute(
                method,
                args,
                &self.subsystem,
                request_context,
                system_resolver,
            )
            .await
            .map_err(|e| match e {
                ModuleRpcExecutionError::Authorization => SubsystemRpcError::Authorization,
                ModuleRpcExecutionError::ExpiredAuthentication => {
                    SubsystemRpcError::ExpiredAuthentication
                }
                ModuleRpcExecutionError::UserDisplayError(msg) => {
                    SubsystemRpcError::UserDisplayError(msg)
                }
                ModuleRpcExecutionError::Internal(msg) => {
                    tracing::error!("Error while resolving RPC operation: {msg}");
                    SubsystemRpcError::UserDisplayError("Internal server error".to_string())
                }
            })?;

        Ok(Some(SubsystemRpcResponse {
            response: QueryResponse {
                body: QueryResponseBody::Json(result),
                headers,
            },
            status_code: StatusCode::OK,
        }))
    }

    fn rpc_schema(&self) -> Option<&RpcSchema> {
        Some(&self.rpc_schema)
    }
}

/// Build the argument sequence for a module method call from validated RPC params.
///
/// Handles both regular arguments (from validated params) and injected arguments
/// (contexts extracted from request, or shims like "Exograph").
async fn build_arg_sequence(
    validated_params: &HashMap<String, Val>,
    method: &subsystem_model_util::module::ModuleMethod,
    subsystem: &ModuleSubsystem,
    request_context: &RequestContext<'_>,
) -> Result<Vec<ModuleArg>, SubsystemRpcError> {
    let mut args = Vec::with_capacity(method.arguments.len());

    for arg in &method.arguments {
        if arg.is_injected {
            let arg_type = &subsystem.module_types[*arg.type_id.innermost()];

            // Check if it's a context type
            let is_context = subsystem
                .contexts
                .iter()
                .any(|(_, context)| context.name == arg_type.name);

            if is_context {
                let context_value = subsystem
                    .extract_context(request_context, &arg_type.name)
                    .await
                    .map_err(SubsystemRpcError::from)?
                    .ok_or_else(|| {
                        tracing::error!("Missing context value for `{}`", arg_type.name);
                        SubsystemRpcError::InternalError
                    })?;
                let json_value: serde_json::Value = context_value.try_into().map_err(|e| {
                    tracing::error!(
                        "Failed to convert context `{}` to JSON: {e:?}",
                        arg_type.name
                    );
                    SubsystemRpcError::InternalError
                })?;
                args.push(ModuleArg::Value(json_value));
            } else {
                // Shim argument (e.g., "Exograph", "Operation")
                args.push(ModuleArg::Shim(arg_type.name.clone()));
            }
        } else if let Some(val) = validated_params.get(&arg.name) {
            let json_value: serde_json::Value = val.clone().try_into().map_err(|e| {
                tracing::error!("Failed to convert parameter `{}` to JSON: {e:?}", arg.name);
                SubsystemRpcError::InternalError
            })?;
            args.push(ModuleArg::Value(json_value));
        } else {
            // Optional parameter not provided — pass null so positional args stay aligned
            args.push(ModuleArg::Value(serde_json::Value::Null));
        }
    }

    Ok(args)
}
