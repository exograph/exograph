use payas_sql::AbstractOperation;

use crate::graphql::execution::query_response::{QueryResponse, QueryResponseBody};

use super::{DatabaseExecutionError, DatabaseSystemContext};

pub async fn resolve_operation<'e>(
    op: &'e AbstractOperation<'e>,
    system_context: &'e DatabaseSystemContext<'e>,
) -> Result<QueryResponse, DatabaseExecutionError> {
    let mut result = system_context
        .database_executor
        .execute(op)
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
