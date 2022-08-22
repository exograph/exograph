use crate::graphql::{execution::system_context::SystemContext, execution_error::ExecutionError};

use async_trait::async_trait;
use payas_model::model::operation::{Query, QueryKind};
use payas_resolver_core::request_context::RequestContext;
use payas_resolver_core::validation::field::ValidatedField;

use payas_resolver_database::{DatabaseQuery, DatabaseSystemContext};
use payas_sql::{AbstractOperation, AbstractPredicate};

use super::{
    data_operation::DataOperation, operation_resolver::OperationResolver,
    service_util::create_service_operation,
};

#[async_trait]
impl<'a> OperationResolver<'a> for Query {
    async fn resolve_operation(
        &'a self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<DataOperation<'a>, ExecutionError> {
        match &self.kind {
            QueryKind::Database(query_params) => {
                let database_query = DatabaseQuery {
                    return_type: &self.return_type,
                    query_params,
                };
                let database_system_context = DatabaseSystemContext {
                    system: &system_context.system,
                    database_executor: &system_context.database_executor,
                    resolve_operation_fn: system_context.resolve_operation_fn(),
                };
                let operation = database_query
                    .compute_select(
                        field,
                        AbstractPredicate::True,
                        &database_system_context,
                        request_context,
                    )
                    .await
                    .map_err(ExecutionError::Database)?;

                Ok(DataOperation::Sql(AbstractOperation::Select(operation)))
            }

            QueryKind::Service { method_id, .. } => {
                create_service_operation(&system_context.system, method_id, field, request_context)
            }
        }
    }
}
