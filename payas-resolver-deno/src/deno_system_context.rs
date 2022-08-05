use payas_model::model::system::ModelSystem;
use payas_resolver_core::ResolveOperationFn;

use super::ClayDenoExecutorPool;

pub struct DenoSystemContext<'s, 'r> {
    pub system: &'s ModelSystem,
    pub deno_execution_pool: &'s ClayDenoExecutorPool,
    pub resolve_operation_fn: ResolveOperationFn<'r>,
}
