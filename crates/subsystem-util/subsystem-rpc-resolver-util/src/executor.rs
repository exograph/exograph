// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! Trait for subsystem-specific execution of module methods.

use async_trait::async_trait;
use common::context::RequestContext;
use subsystem_model_util::module::ModuleMethod;
use subsystem_model_util::subsystem::ModuleSubsystem;
use thiserror::Error;

/// A single argument to pass to a module method.
pub enum ModuleArg {
    /// A regular argument (serialized JSON value).
    Value(serde_json::Value),
    /// An injected shim argument (e.g., "Exograph", "Operation").
    Shim(String),
}

/// Trait that subsystem-specific executors (Deno, WASM) implement to run module methods.
#[async_trait]
pub trait ModuleRpcExecutor: Send + Sync {
    /// Execute a module method with the given arguments.
    ///
    /// Returns a tuple of (result JSON, response headers).
    async fn execute<'a>(
        &'a self,
        method: &'a ModuleMethod,
        args: Vec<ModuleArg>,
        subsystem: &'a ModuleSubsystem,
        request_context: &'a RequestContext<'a>,
        system_resolver: &'a core_resolver::system_resolver::GraphQLSystemResolver,
    ) -> Result<(serde_json::Value, Vec<(String, String)>), ModuleRpcExecutionError>;
}

#[derive(Error, Debug)]
pub enum ModuleRpcExecutionError {
    #[error("Authorization denied")]
    Authorization,

    #[error("Expired authentication")]
    ExpiredAuthentication,

    #[error("{0}")]
    UserDisplayError(String),

    #[error("{0}")]
    Internal(String),
}
