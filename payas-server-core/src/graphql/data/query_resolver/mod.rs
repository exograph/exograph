use crate::graphql::{execution::system_context::SystemContext, execution_error::ExecutionError};

use async_trait::async_trait;
use payas_model::model::operation::{Interceptors, Query, QueryKind};
use payas_resolver_core::request_context::RequestContext;
use payas_resolver_core::validation::field::ValidatedField;

use payas_resolver_database::{DatabaseExecutionError, DatabaseQuery, DatabaseSystemContext};
use payas_sql::{AbstractOperation, AbstractPredicate};

use super::{
    deno::deno_resolver::DenoOperation, operation_mapper::OperationResolverResult,
    operation_resolver::OperationResolver,
};

// TODO: deal with panics at the type level

#[async_trait]
impl<'a> OperationResolver<'a> for Query {
    async fn resolve_operation(
        &'a self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<OperationResolverResult<'a>, ExecutionError> {
        Ok(match &self.kind {
            QueryKind::Database(query_params) => {
                let database_query = DatabaseQuery {
                    return_type: &self.return_type,
                    query_params,
                };
                let database_system_context = DatabaseSystemContext {
                    system: &system_context.system,
                    database_executor: &system_context.database_executor,
                    resolve: system_context.curried_resolve(),
                };
                let operation = database_query
                    .compute_select(
                        field,
                        AbstractPredicate::True,
                        &database_system_context,
                        request_context,
                    )
                    .await
                    .map_err(|database_execution_error| match database_execution_error {
                        DatabaseExecutionError::Authorization => ExecutionError::Authorization,
                        DatabaseExecutionError::Generic(messages) => {
                            ExecutionError::Generic(messages)
                        }
                        e => ExecutionError::Database(e),
                    })?;

                OperationResolverResult::SQLOperation(AbstractOperation::Select(operation))
            }

            QueryKind::Service { method_id, .. } => {
                OperationResolverResult::DenoOperation(DenoOperation(method_id.unwrap()))
            }
        })
    }

    fn interceptors(&self) -> &Interceptors {
        &self.interceptors
    }

    fn name(&self) -> &str {
        &self.name
    }
}
