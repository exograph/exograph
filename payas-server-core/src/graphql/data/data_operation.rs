use payas_resolver_core::QueryResponse;
use payas_resolver_database::DatabaseSystemContext;

use payas_sql::AbstractOperation;

use crate::graphql::{execution::system_context::SystemContext, execution_error::ExecutionError};

use payas_resolver_deno::{DenoOperation, DenoSystemContext};

#[allow(clippy::large_enum_variant)]
pub enum DataOperation<'a> {
    SQLOperation(AbstractOperation<'a>),
    DenoOperation(DenoOperation<'a>),
}

impl<'a> DataOperation<'a> {
    pub async fn execute(
        &self,
        system_context: &'a SystemContext,
    ) -> Result<QueryResponse, ExecutionError> {
        let resolve_operation_fn = system_context.resolve_operation_fn();
        match self {
            DataOperation::SQLOperation(abstract_operation) => {
                let database_system_context = DatabaseSystemContext {
                    system: &system_context.system,
                    database_executor: &system_context.database_executor,
                    resolve_operation_fn,
                };

                payas_resolver_database::resolve_operation(
                    abstract_operation,
                    database_system_context,
                )
                .await
                .map_err(ExecutionError::Database)
            }

            DataOperation::DenoOperation(operation) => {
                let deno_system_context = DenoSystemContext {
                    system: &system_context.system,
                    deno_execution_pool: &system_context.deno_execution_pool,
                    resolve_operation_fn,
                };

                operation
                    .execute(&deno_system_context)
                    .await
                    .map_err(ExecutionError::Deno)
            }
        }
    }
}
