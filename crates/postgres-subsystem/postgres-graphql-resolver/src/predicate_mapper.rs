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
use exo_sql::AbstractPredicate;

use postgres_core_model::predicate::PredicateParameter;
use postgres_graphql_model::subsystem::PostgresGraphQLSubsystem;

use crate::{
    sql_mapper::{SQLMapper, extract_and_map},
    util::Arguments,
};

use postgres_core_resolver::postgres_execution_error::PostgresExecutionError;

#[derive(Debug)]
struct PredicateParamInput<'a> {
    pub param: &'a PredicateParameter,
}

#[async_trait]
impl<'a> SQLMapper<'a, AbstractPredicate> for PredicateParamInput<'a> {
    async fn to_sql(
        self,
        argument: &'a Val,
        subsystem: &'a PostgresGraphQLSubsystem,
        request_context: &'a RequestContext<'a>,
    ) -> Result<AbstractPredicate, PostgresExecutionError> {
        postgres_core_resolver::predicate_mapper::compute_predicate(
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

pub async fn compute_predicate<'a>(
    params: &'a [&'a PredicateParameter],
    arguments: &'a Arguments,
    subsystem: &'a PostgresGraphQLSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<AbstractPredicate, PostgresExecutionError> {
    let predicates = futures::future::try_join_all(params.iter().map(|param| async {
        extract_and_map(
            PredicateParamInput { param },
            arguments,
            subsystem,
            request_context,
        )
        .await
    }))
    .await?;

    let predicates = predicates.into_iter().flatten();

    Ok(predicates.fold(AbstractPredicate::True, |acc, predicate| {
        AbstractPredicate::and(acc, predicate)
    }))
}
