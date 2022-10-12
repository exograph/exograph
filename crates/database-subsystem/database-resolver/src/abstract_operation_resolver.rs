use payas_sql::AbstractOperation;

use core_resolver::{request_context::RequestContext, QueryResponse, QueryResponseBody};
use postgres_types::FromSqlOwned;
use tokio_postgres::Row;

use crate::{
    database_execution_error::DatabaseExecutionError,
    plugin::subsystem_resolver::DatabaseSubsystemResolver,
};

pub async fn resolve_operation<'e>(
    op: &AbstractOperation<'e>,
    subsystem_resolver: &'e DatabaseSubsystemResolver,
    request_context: &'e RequestContext<'e>,
) -> Result<QueryResponse, DatabaseExecutionError> {
    let ctx = request_context.get_base_context();
    let mut tx = ctx.transaction_holder.try_lock().unwrap();

    let mut result = subsystem_resolver
        .executor
        .execute(op, &mut tx)
        .await
        .map_err(DatabaseExecutionError::Database)?;

    let body = if result.len() == 1 {
        let string_result = extractor(result.swap_remove(0))?;
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

fn extractor<T: FromSqlOwned>(row: Row) -> Result<T, DatabaseExecutionError> {
    match row.try_get(0) {
        Ok(col) => Ok(col),
        Err(err) => Err(DatabaseExecutionError::EmptyRow(err)),
    }
}
