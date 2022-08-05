use payas_model::model::system::ModelSystem;
use payas_resolver_core::{ResolveFn, ResolveFnOwned};

use super::ClayDenoExecutorPool;

pub struct DenoSystemContext<'s, 'r> {
    pub system: &'s ModelSystem,
    pub deno_execution_pool: &'s ClayDenoExecutorPool,
    pub resolve_query_fn: ResolveFn<'r>,
    pub resolve_query_owned_fn: ResolveFnOwned<'r>,
}
