// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_trait::async_trait;
use common::context::RequestContext;
use common::value::Val;
use exo_sql::{Limit, Offset};
use postgres_model::{
    limit_offset::{LimitParameter, OffsetParameter},
    subsystem::PostgresSubsystem,
};

use super::{postgres_execution_error::PostgresExecutionError, sql_mapper::SQLMapper};

fn cast_to_i64(argument: &Val) -> Result<i64, PostgresExecutionError> {
    match argument {
        Val::Number(n) => n
            .as_i64()
            .ok_or_else(|| PostgresExecutionError::Generic(format!("Could not cast {n} to i64"))),
        _ => Err(PostgresExecutionError::Generic("Not a number".into())),
    }
}

#[async_trait]
impl<'a> SQLMapper<'a, Limit> for &LimitParameter {
    async fn to_sql(
        self,
        argument: &'a Val,
        _subsystem: &'a PostgresSubsystem,
        _request_context: &'a RequestContext<'a>,
    ) -> Result<Limit, PostgresExecutionError> {
        cast_to_i64(argument).map(Limit)
    }

    fn param_name(&self) -> &str {
        &self.name
    }
}

#[async_trait]
impl<'a> SQLMapper<'a, Offset> for &OffsetParameter {
    async fn to_sql(
        self,
        argument: &'a Val,
        _subsystem: &'a PostgresSubsystem,
        _request_context: &'a RequestContext<'a>,
    ) -> Result<Offset, PostgresExecutionError> {
        cast_to_i64(argument).map(Offset)
    }

    fn param_name(&self) -> &str {
        &self.name
    }
}
