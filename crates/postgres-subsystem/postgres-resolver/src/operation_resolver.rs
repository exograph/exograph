use async_trait::async_trait;
use core_resolver::request_context::RequestContext;
use core_resolver::validation::field::ValidatedField;
use payas_sql::AbstractOperation;
use postgres_model::model::ModelPostgresSystem;

use crate::postgres_execution_error::PostgresExecutionError;

#[async_trait]
pub trait OperationResolver {
    async fn resolve<'a>(
        &'a self,
        field: &'a ValidatedField,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a ModelPostgresSystem,
    ) -> Result<AbstractOperation<'a>, PostgresExecutionError>;
}
