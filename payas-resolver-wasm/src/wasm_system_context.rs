use payas_deno_model::model::ModelServiceSystem;
use payas_resolver_core::ResolveOperationFn;

use super::WasmExecutorPool;

pub struct WasmSystemContext<'r> {
    pub system: &'r ModelServiceSystem,
    pub executor_pool: &'r WasmExecutorPool,
    pub resolve_operation_fn: ResolveOperationFn<'r>,
}
