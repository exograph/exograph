use async_trait::async_trait;
use core_plugin_interface::core_resolver::{
    request_context::RequestContext, validation::field::ValidatedField,
};
use payas_sql::{AbstractOperation, AbstractSelect};
use postgres_model::model::ModelPostgresSystem;

use crate::postgres_execution_error::PostgresExecutionError;

#[async_trait]
pub trait OperationSelectionResolver {
    async fn resolve_select<'a>(
        &'a self,
        field: &'a ValidatedField,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a ModelPostgresSystem,
    ) -> Result<AbstractSelect<'a>, PostgresExecutionError>;
}

#[async_trait]
pub trait OperationResolver {
    async fn resolve<'a>(
        &'a self,
        field: &'a ValidatedField,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a ModelPostgresSystem,
    ) -> Result<AbstractOperation<'a>, PostgresExecutionError>;
}

#[async_trait]
impl<T: OperationSelectionResolver + Send + Sync> OperationResolver for T {
    async fn resolve<'a>(
        &'a self,
        field: &'a ValidatedField,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a ModelPostgresSystem,
    ) -> Result<AbstractOperation<'a>, PostgresExecutionError> {
        self.resolve_select(field, request_context, subsystem)
            .await
            .map(AbstractOperation::Select)
    }
}
