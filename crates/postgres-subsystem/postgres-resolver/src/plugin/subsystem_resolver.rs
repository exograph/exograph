// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{
    abstract_operation_resolver::resolve_operation, operation_resolver::OperationResolver,
    postgres_execution_error::PostgresExecutionError,
};
use async_graphql_parser::types::{FieldDefinition, OperationType, TypeDefinition};
use async_trait::async_trait;
use core_plugin_interface::{
    core_resolver::{
        plugin::{SubsystemResolutionError, SubsystemResolver},
        request_context::RequestContext,
        system_resolver::SystemResolver,
        validation::field::ValidatedField,
        InterceptedOperation, QueryResponse,
    },
    interception::InterceptorIndex,
};
use exo_sql::DatabaseExecutor;
use postgres_model::subsystem::PostgresSubsystem;

pub struct PostgresSubsystemResolver {
    pub id: &'static str,
    pub subsystem: PostgresSubsystem,
    pub executor: DatabaseExecutor,
}

#[async_trait]
impl SubsystemResolver for PostgresSubsystemResolver {
    fn id(&self) -> &'static str {
        self.id
    }

    async fn resolve<'a>(
        &'a self,
        field: &'a ValidatedField,
        operation_type: OperationType,
        request_context: &'a RequestContext<'a>,
        _system_resolver: &'a SystemResolver,
    ) -> Result<Option<QueryResponse>, SubsystemResolutionError> {
        let operation_name = &field.name;

        let operation = match operation_type {
            OperationType::Query => match self.subsystem.pk_queries.get_by_key(operation_name) {
                Some(query) => Some(query.resolve(field, request_context, &self.subsystem).await),
                None => match self.subsystem.collection_queries.get_by_key(operation_name) {
                    Some(query) => {
                        Some(query.resolve(field, request_context, &self.subsystem).await)
                    }
                    None => match self.subsystem.aggregate_queries.get_by_key(operation_name) {
                        Some(query) => {
                            Some(query.resolve(field, request_context, &self.subsystem).await)
                        }
                        None => None,
                    },
                },
            },
            OperationType::Mutation => {
                let mutation = self.subsystem.mutations.get_by_key(operation_name);

                match mutation {
                    Some(mutation) => Some(
                        mutation
                            .resolve(field, request_context, &self.subsystem)
                            .await,
                    ),
                    None => None,
                }
            }
            OperationType::Subscription => Some(Err(PostgresExecutionError::Generic(
                "Subscriptions are not supported".to_string(),
            ))),
        };

        match operation {
            Some(Ok(operation)) => Ok(Some(
                resolve_operation(&operation, self, request_context).await?,
            )),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    async fn invoke_interceptor<'a>(
        &'a self,
        _interceptor_index: InterceptorIndex,
        _intercepted_operation: &'a InterceptedOperation<'a>,
        _request_context: &'a RequestContext<'a>,
        _system_resolver: &'a SystemResolver,
    ) -> Result<Option<QueryResponse>, SubsystemResolutionError> {
        Err(SubsystemResolutionError::NoInterceptorFound)
    }

    fn schema_queries(&self) -> Vec<FieldDefinition> {
        self.subsystem.schema_queries()
    }

    fn schema_mutations(&self) -> Vec<FieldDefinition> {
        self.subsystem.schema_mutations()
    }

    fn schema_types(&self) -> Vec<TypeDefinition> {
        self.subsystem.schema_types()
    }
}

impl From<PostgresExecutionError> for SubsystemResolutionError {
    fn from(e: PostgresExecutionError) -> Self {
        match e {
            PostgresExecutionError::Authorization => SubsystemResolutionError::Authorization,
            _ => SubsystemResolutionError::UserDisplayError(e.user_error_message()),
        }
    }
}
