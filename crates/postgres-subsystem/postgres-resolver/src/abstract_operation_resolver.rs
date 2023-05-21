// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use exo_sql::AbstractOperation;

use core_plugin_interface::core_resolver::{
    context::RequestContext, QueryResponse, QueryResponseBody,
};
use postgres_types::FromSqlOwned;
use tokio_postgres::Row;

use crate::{
    plugin::subsystem_resolver::PostgresSubsystemResolver,
    postgres_execution_error::PostgresExecutionError,
};

pub async fn resolve_operation<'e>(
    op: &AbstractOperation,
    subsystem_resolver: &'e PostgresSubsystemResolver,
    request_context: &'e RequestContext<'e>,
) -> Result<QueryResponse, PostgresExecutionError> {
    let ctx = request_context.get_base_context();
    let mut tx = ctx.transaction_holder.try_lock().unwrap();

    let mut result = subsystem_resolver
        .executor
        .execute(op, &mut tx, &subsystem_resolver.subsystem.database)
        .await
        .map_err(PostgresExecutionError::Postgres)?;

    let body = if result.len() == 1 {
        let string_result = extractor(result.swap_remove(0))?;
        Ok(QueryResponseBody::Raw(Some(string_result)))
    } else if result.is_empty() {
        Ok(QueryResponseBody::Raw(None))
    } else {
        Err(PostgresExecutionError::NonUniqueResult(result.len()))
    }?;

    Ok(QueryResponse {
        body,
        headers: vec![], // we shouldn't get any HTTP headers from a SQL op
    })
}

fn extractor<T: FromSqlOwned>(row: Row) -> Result<T, PostgresExecutionError> {
    match row.try_get(0) {
        Ok(col) => Ok(col),
        Err(err) => Err(PostgresExecutionError::EmptyRow(err)),
    }
}
