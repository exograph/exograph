use payas_model::model::system::ModelSystem;
use payas_resolver_core::ResolveOperationFn;
use payas_sql::DatabaseExecutor;

pub struct DatabaseSystemContext<'a> {
    pub system: &'a ModelSystem,
    pub database_executor: &'a DatabaseExecutor,
    pub resolve_operation_fn: ResolveOperationFn<'a>,
}
