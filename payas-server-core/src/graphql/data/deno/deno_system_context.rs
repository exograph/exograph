use payas_model::model::system::ModelSystem;
use payas_resolver_core::{ResolveFn, ResolveFnOwnedUnderlying};

use super::ClayDenoExecutorPool;

pub struct DenoSystemContext<'s, 'a> {
    pub system: &'a ModelSystem,
    pub deno_execution_pool: &'a ClayDenoExecutorPool,
    pub resolve_query_fn: &'s ResolveFn<'a>,
    pub resolve_query_owned_fn: &'s ResolveFnOwnedUnderlying<'a>,
}
