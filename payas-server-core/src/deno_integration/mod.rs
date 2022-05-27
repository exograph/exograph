pub mod clay_execution;
pub mod claytip_ops;

pub use clay_execution::{
    clay_config, ClayCallbackProcessor, FnClaytipExecuteQuery, FnClaytipInterceptorProceed,
};
pub use claytip_ops::InterceptedOperationName;
use payas_deno::DenoExecutorPool;
pub type ClayDenoExecutorPool = DenoExecutorPool<
    Option<InterceptedOperationName>,
    clay_execution::RequestFromDenoMessage,
    clay_execution::ClaytipMethodResponse,
>;
