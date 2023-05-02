// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_trait::async_trait;
use core_plugin_interface::core_resolver::context::RequestContext;
use core_plugin_interface::core_resolver::value::Val;
use postgres_model::subsystem::PostgresSubsystem;

use crate::util::{find_arg, Arguments};

use super::postgres_execution_error::PostgresExecutionError;
pub(crate) enum SQLOperationKind {
    Create,
    Retrieve,
    Update,
    Delete,
}

#[async_trait]
pub(crate) trait SQLMapper<'a, R> {
    async fn to_sql(
        self,
        argument: &'a Val,
        subsystem: &'a PostgresSubsystem,
        request_context: &'a RequestContext<'a>,
    ) -> Result<R, PostgresExecutionError>;

    fn param_name(&self) -> &str;
}

pub(crate) async fn extract_and_map<'a, P, R>(
    param: P,
    arguments: &'a Arguments,
    subsystem: &'a PostgresSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<Option<R>, PostgresExecutionError>
where
    P: SQLMapper<'a, R>,
{
    let argument_value = find_arg(arguments, param.param_name());

    match argument_value {
        None => Ok(None),
        Some(argument_value) => Some(
            param
                .to_sql(argument_value, subsystem, request_context)
                .await,
        )
        .transpose(),
    }
}
