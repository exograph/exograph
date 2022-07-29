use crate::graphql::execution::query_response::QueryResponse;
use crate::graphql::execution_error::ExecutionError;
use crate::graphql::request_context::RequestContext;

use crate::graphql::{execution::system_context::SystemContext, validation::field::ValidatedField};

use payas_model::model::{mapped_arena::SerializableSlabIndex, service::ServiceMethod};
use payas_sql::AbstractOperation;

use super::database::{DatabaseExecutionError, DatabaseSystemContext};
use super::deno::DenoExecutionError;

#[allow(clippy::large_enum_variant)]
pub enum OperationResolverResult<'a> {
    SQLOperation(AbstractOperation<'a>),
    DenoOperation(DenoOperation),
}

pub struct DenoOperation(pub SerializableSlabIndex<ServiceMethod>);

impl<'a> OperationResolverResult<'a> {
    pub async fn execute(
        &self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<QueryResponse, ExecutionError> {
        match self {
            OperationResolverResult::SQLOperation(abstract_operation) => {
                let database_system_context = DatabaseSystemContext {
                    system: &system_context.system,
                    database_executor: &system_context.database_executor,
                };

                super::database::resolve_operation(abstract_operation, &database_system_context)
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
