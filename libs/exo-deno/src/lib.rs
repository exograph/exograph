// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

/// This code has no concept of Exograph.
///
/// Module to encapsulate the logic creating a Deno module that supports
/// embedding.
///
pub mod deno_error;
pub mod deno_executor;
pub mod deno_executor_pool;
pub mod deno_module;

pub use deno_executor_pool::DenoExecutorPool;
pub use deno_module::{Arg, DenoModule, DenoModuleSharedState, UserCode};

mod deno_actor;
mod embedded_module_loader;
#[cfg(feature = "typescript-loader")]
mod typescript_module_loader;

pub use deno_core;

#[cfg(test)]
use ctor::ctor;

#[cfg(test)]
#[ctor]
// Make sure deno runtime is initialized in the main thread in test executables.
fn init_deno_runtime() {
    deno_core::JsRuntime::init_platform(None);
}
