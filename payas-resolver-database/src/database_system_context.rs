use payas_model::model::system::ModelSystem;
use payas_resolver_core::ResolveFn;
use payas_sql::DatabaseExecutor;

pub struct DatabaseSystemContext<'a> {
    pub system: &'a ModelSystem,
    pub database_executor: &'a DatabaseExecutor,
    pub resolve: ResolveFn<'a>,
}
