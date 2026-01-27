// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

//! GraphQL-specific order-by mapper that delegates to the core mapper.

use async_trait::async_trait;

use crate::sql_mapper::SQLMapper;
use common::context::RequestContext;
use common::value::Val;
use exo_sql::AbstractOrderBy;
use postgres_core_model::order::OrderByParameter;
use postgres_core_resolver::order_by_mapper::compute_order_by;
use postgres_core_resolver::postgres_execution_error::PostgresExecutionError;

use postgres_graphql_model::subsystem::PostgresGraphQLSubsystem;

pub(crate) struct OrderByParameterInput<'a> {
    pub param: &'a OrderByParameter,
}

#[async_trait]
impl<'a> SQLMapper<'a, AbstractOrderBy> for OrderByParameterInput<'a> {
    async fn to_sql(
        self,
        argument: &'a Val,
        subsystem: &'a PostgresGraphQLSubsystem,
        request_context: &'a RequestContext<'a>,
    ) -> Result<AbstractOrderBy, PostgresExecutionError> {
        compute_order_by(
            self.param,
            argument,
            &subsystem.core_subsystem,
            request_context,
        )
        .await
    }

    fn param_name(&self) -> &str {
        &self.param.name
    }
}
