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
use core_plugin_interface::core_resolver::validation::field::ValidatedField;
use exo_sql::{AbstractOperation, AbstractPredicate, AbstractSelect};
use postgres_graphql_model::subsystem::PostgresGraphQLSubsystem;

use postgres_core_resolver::postgres_execution_error::PostgresExecutionError;

#[async_trait]
pub trait OperationSelectionResolver {
    async fn resolve_select<'a>(
        &'a self,
        field: &'a ValidatedField,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresGraphQLSubsystem,
    ) -> Result<AbstractSelect, PostgresExecutionError>;
}

pub struct OperationResolution<O> {
    /// The precheck predicates to be executed before the operation is executed.
    /// Each predicate must return a single row to indicate passing the precheck (in other words, returning zero rows indicates failure).
    pub precheck_predicates: Vec<AbstractPredicate>,
    /// The operation to be executed if the precheck passes
    pub operation: O,
}

#[async_trait]
pub trait OperationResolver {
    async fn resolve<'a>(
        &'a self,
        field: &'a ValidatedField,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresGraphQLSubsystem,
    ) -> Result<OperationResolution<AbstractOperation>, PostgresExecutionError>;
}

#[async_trait]
impl<T: OperationSelectionResolver + Send + Sync> OperationResolver for T {
    async fn resolve<'a>(
        &'a self,
        field: &'a ValidatedField,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresGraphQLSubsystem,
    ) -> Result<OperationResolution<AbstractOperation>, PostgresExecutionError> {
        self.resolve_select(field, request_context, subsystem)
            .await
            .map(|select| OperationResolution {
                precheck_predicates: vec![],
                operation: AbstractOperation::Select(select),
            })
    }
}
