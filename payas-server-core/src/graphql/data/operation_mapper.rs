use crate::graphql::execution::query_response::{QueryResponse, QueryResponseBody};
use crate::graphql::execution_error::{DatabaseExecutionError, ExecutionError};
use crate::graphql::request_context::RequestContext;

use crate::graphql::{execution::system_context::SystemContext, validation::field::ValidatedField};

use payas_model::model::{mapped_arena::SerializableSlabIndex, service::ServiceMethod};
use payas_sql::AbstractOperation;

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
                let mut result = system_context
                    .database_executor
                    .execute(abstract_operation)
                    .await
                    .map_err(DatabaseExecutionError::Database)?;

                let body = if result.len() == 1 {
                    let string_result = crate::graphql::database::extractor(result.swap_remove(0))?;
                    Ok(QueryResponseBody::Raw(Some(string_result)))
                } else if result.is_empty() {
                    Ok(QueryResponseBody::Raw(None))
                } else {
                    Err(DatabaseExecutionError::NonUniqueResult(result.len()))
                }?;

                Ok(QueryResponse {
                    body,
                    headers: vec![], // we shouldn't get any HTTP headers from a SQL op
                })
            }

            OperationResolverResult::DenoOperation(operation) => {
                operation
                    .execute(field, system_context, request_context)
                    .await
            }
        }
    }
}
