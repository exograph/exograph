use payas_resolver_core::ResolveOperationFn;
use payas_service_model::model::ModelServiceSystem;

use super::WasmExecutorPool;

pub struct WasmSystemContext<'r> {
    pub system: &'r ModelServiceSystem,
    pub executor_pool: &'r WasmExecutorPool,
    pub resolve_operation_fn: ResolveOperationFn<'r>,
}
