use async_trait::async_trait;

use payas_sql::AbstractOperation;

use crate::{
    graphql::{
        execution::{
            field_resolver::FieldResolver,
            query_response::{QueryResponse, QueryResponseBody},
        },
        validation::field::ValidatedField,
    },
    request_context::RequestContext,
    SystemContext,
};

use super::DatabaseExecutionError;

#[async_trait]
impl<'a> FieldResolver<'static, QueryResponse, DatabaseExecutionError, SystemContext>
    for AbstractOperation<'a>
{
    async fn resolve_field<'e>(
        &'e self,
        _field: &ValidatedField,
        system_context: &'e SystemContext,
        _request_context: &'e RequestContext<'e>,
    ) -> Result<QueryResponse, DatabaseExecutionError> {
        let mut result = system_context
            .database_executor
            .execute(self)
            .await
            .map_err(DatabaseExecutionError::Database)?;

        let body = if result.len() == 1 {
            let string_result = super::extractor(result.swap_remove(0))?;
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
}
