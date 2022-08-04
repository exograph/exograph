use payas_resolver_core::validation::field::ValidatedField;
use payas_resolver_core::{request_context::RequestContext, QueryResponse};
use payas_resolver_database::{DatabaseExecutionError, DatabaseSystemContext};

use payas_sql::AbstractOperation;

use crate::graphql::{execution::system_context::SystemContext, execution_error::ExecutionError};

use super::deno::{deno_resolver::DenoOperation, DenoExecutionError};

#[allow(clippy::large_enum_variant)]
pub enum OperationResolverResult<'a> {
    SQLOperation(AbstractOperation<'a>),
    DenoOperation(DenoOperation),
}

impl<'a> OperationResolverResult<'a> {
    pub async fn execute(
        &self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<QueryResponse, ExecutionError> {
        let resolve = system_context.curried_resolve();
        match self {
            OperationResolverResult::SQLOperation(abstract_operation) => {
                let database_system_context = DatabaseSystemContext {
                    system: &system_context.system,
                    database_executor: &system_context.database_executor,
                    resolve,
                };

                payas_resolver_database::resolve_operation(
                    abstract_operation,
                    database_system_context,
                )
                .await
                .map_err(|e| match e {
                    DatabaseExecutionError::Authorization => ExecutionError::Authorization,
                    e => ExecutionError::Database(e),
                })
            }

            OperationResolverResult::DenoOperation(operation) => operation
                .execute(field, system_context, request_context)
                .await
                .map_err(|e| match e {
                    DenoExecutionError::Authorization => ExecutionError::Authorization,
                    e => ExecutionError::Deno(e),
                }),
        }
    }
}
