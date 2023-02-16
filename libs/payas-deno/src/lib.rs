/// This code has no concept of Claytip.
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
