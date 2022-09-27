use payas_core_resolver::ResolveOperationFn;
use payas_wasm_model::model::ModelWasmSystem;

use super::WasmExecutorPool;

pub struct WasmSystemContext<'r> {
    pub system: &'r ModelWasmSystem,
    pub executor_pool: &'r WasmExecutorPool,
    pub resolve_operation_fn: ResolveOperationFn<'r>,
}
