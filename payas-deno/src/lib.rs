mod clay;
mod generic;

pub use clay::clay_execution::{
    clay_config, ClayCallbackProcessor, FnClaytipExecuteQuery, FnClaytipInterceptorProceed,
};
pub use clay::claytip_ops::InterceptedOperationName;
pub use generic::deno_executor_pool::DenoExecutorPool;
pub use generic::deno_module::{Arg, DenoModule, DenoModuleSharedState, UserCode};

pub type ClayDenoExecutorPool = DenoExecutorPool<
    Option<InterceptedOperationName>,
    clay::clay_execution::RequestFromDenoMessage,
>;
