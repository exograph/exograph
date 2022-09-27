use payas_core_resolver::ResolveOperationFn;
use payas_database_model::model::ModelDatabaseSystem;
use payas_sql::DatabaseExecutor;

pub struct DatabaseSystemContext<'a> {
    pub system: &'a ModelDatabaseSystem,
    pub database_executor: &'a DatabaseExecutor,
    pub resolve_operation_fn: ResolveOperationFn<'a>,
}
