use payas_model::model::system::ModelSystem;
use payas_resolver_core::ResolveOperationFn;

use super::WasmExecutorPool;

pub struct WasmSystemContext<'r> {
    pub system: &'r ModelSystem,
    pub executor_pool: &'r WasmExecutorPool,
    pub resolve_operation_fn: ResolveOperationFn<'r>,
}
