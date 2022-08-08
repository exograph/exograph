use payas_model::model::system::ModelSystem;
use payas_resolver_core::ResolveOperationFn;

use super::ClayDenoExecutorPool;

pub struct DenoSystemContext<'r> {
    pub system: &'r ModelSystem,
    pub deno_execution_pool: &'r ClayDenoExecutorPool,
    pub resolve_operation_fn: ResolveOperationFn<'r>,
}
