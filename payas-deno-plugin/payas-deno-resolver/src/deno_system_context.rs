use payas_core_resolver::ResolveOperationFn;
use payas_deno_model::model::ModelDenoSystem;

use super::plugin::ClayDenoExecutorPool;

pub struct DenoSystemContext<'r> {
    pub system: &'r ModelDenoSystem,
    pub deno_execution_pool: &'r ClayDenoExecutorPool,
    pub resolve_operation_fn: ResolveOperationFn<'r>,
}
