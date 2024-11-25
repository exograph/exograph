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
use exo_sql::{AbstractOperation, AbstractSelect};
use postgres_model::subsystem::PostgresSubsystem;

use crate::postgres_execution_error::PostgresExecutionError;

#[async_trait]
pub trait OperationSelectionResolver {
    async fn resolve_select<'a>(
        &'a self,
        field: &'a ValidatedField,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresSubsystem,
    ) -> Result<AbstractSelect, PostgresExecutionError>;
}

#[async_trait]
pub trait OperationResolver {
    async fn resolve<'a>(
        &'a self,
        field: &'a ValidatedField,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresSubsystem,
    ) -> Result<AbstractOperation, PostgresExecutionError>;
}

#[async_trait]
impl<T: OperationSelectionResolver + Send + Sync> OperationResolver for T {
    async fn resolve<'a>(
        &'a self,
        field: &'a ValidatedField,
        request_context: &'a RequestContext<'a>,
        subsystem: &'a PostgresSubsystem,
    ) -> Result<AbstractOperation, PostgresExecutionError> {
        self.resolve_select(field, request_context, subsystem)
            .await
            .map(AbstractOperation::Select)
    }
}
