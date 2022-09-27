use payas_deno_model::model::ModelServiceSystem;
use payas_resolver_core::ResolveOperationFn;

use super::ClayDenoExecutorPool;

pub struct DenoSystemContext<'r> {
    pub system: &'r ModelServiceSystem,
    pub deno_execution_pool: &'r ClayDenoExecutorPool,
    pub resolve_operation_fn: ResolveOperationFn<'r>,
}
