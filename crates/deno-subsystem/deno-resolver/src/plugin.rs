// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_graphql_parser::types::{FieldDefinition, OperationType, TypeDefinition};
use async_trait::async_trait;

use core_plugin_interface::{
    core_model::mapped_arena::SerializableSlabIndex,
    core_resolver::{
        context::RequestContext,
        exograph_execute_query,
        plugin::{SubsystemResolutionError, SubsystemResolver},
        system_resolver::SystemResolver,
        validation::field::ValidatedField,
        InterceptedOperation, QueryResponse, QueryResponseBody,
    },
    interception::InterceptorIndex,
    interface::{SubsystemLoader, SubsystemLoadingError},
    system_serializer::SystemSerializer,
};

use deno_model::{module::ModuleMethod, subsystem::DenoSubsystem};
use exo_deno::DenoExecutorPool;

use super::{
    deno_execution_error::DenoExecutionError,
    deno_operation::DenoOperation,
    exo_execution::{exo_config, ExographMethodResponse, RequestFromDenoMessage},
    exograph_ops::InterceptedOperationInfo,
};

pub type ExoDenoExecutorPool = DenoExecutorPool<
    Option<InterceptedOperationInfo>,
    RequestFromDenoMessage,
    ExographMethodResponse,
>;

pub struct DenoSubsystemLoader {}

impl SubsystemLoader for DenoSubsystemLoader {
    fn id(&self) -> &'static str {
        "deno"
    }

    fn init<'a>(
        &self,
        serialized_subsystem: Vec<u8>,
    ) -> Result<Box<dyn SubsystemResolver + Send + Sync>, SubsystemLoadingError> {
        let subsystem = DenoSubsystem::deserialize(serialized_subsystem)?;

        let executor = DenoExecutorPool::new_from_config(exo_config());

        Ok(Box::new(DenoSubsystemResolver {
            id: self.id(),
            subsystem,
            executor,
        }))
    }
}

pub struct DenoSubsystemResolver {
    pub id: &'static str,
    pub subsystem: DenoSubsystem,
    pub executor: ExoDenoExecutorPool,
}

#[async_trait]
impl SubsystemResolver for DenoSubsystemResolver {
    fn id(&self) -> &'static str {
        self.id
    }

    async fn resolve<'a>(
        &'a self,
        operation: &'a ValidatedField,
        operation_type: OperationType,
        request_context: &'a RequestContext<'a>,
        system_resolver: &'a SystemResolver,
    ) -> Result<Option<QueryResponse>, SubsystemResolutionError> {
        let operation_name = &operation.name;

        let deno_operation = match operation_type {
            OperationType::Query => {
                let query = self.subsystem.queries.get_by_key(operation_name);
                query.map(|query| {
                    create_deno_operation(
                        &self.subsystem,
                        &query.method_id,
                        operation,
                        request_context,
                        self,
                        system_resolver,
                    )
                })
            }
            OperationType::Mutation => {
                let mutation = self.subsystem.mutations.get_by_key(operation_name);
                mutation.map(|mutation| {
                    create_deno_operation(
                        &self.subsystem,
                        &mutation.method_id,
                        operation,
                        request_context,
                        self,
                        system_resolver,
                    )
                })
            }
            OperationType::Subscription => Some(Err(DenoExecutionError::Generic(
                "Subscriptions are not supported".to_string(),
            ))),
        };

        match deno_operation {
            Some(Ok(operation)) => Ok(Some(operation.execute().await?)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    async fn invoke_interceptor<'a>(
        &'a self,
        interceptor_index: InterceptorIndex,
        intercepted_operation: &'a InterceptedOperation<'a>,
        request_context: &'a RequestContext<'a>,
        system_resolver: &'a SystemResolver,
    ) -> Result<Option<QueryResponse>, SubsystemResolutionError> {
        let interceptor =
            &self.subsystem.interceptors[SerializableSlabIndex::from_idx(interceptor_index.0)];

        let exograph_execute_query = exograph_execute_query!(system_resolver, request_context);
        let (result, response) = super::interceptor_execution::execute_interceptor(
            interceptor,
            self,
            request_context,
            &exograph_execute_query,
            intercepted_operation,
        )
        .await?;

        let body = match result {
            serde_json::Value::String(value) => QueryResponseBody::Raw(Some(value)),
            _ => QueryResponseBody::Json(result),
        };

        Ok(Some(QueryResponse {
            body,
            headers: response.map(|r| r.headers).unwrap_or_default(),
        }))
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

pub(crate) fn create_deno_operation<'a>(
    system: &'a DenoSubsystem,
    method_id: &Option<SerializableSlabIndex<ModuleMethod>>,
    field: &'a ValidatedField,
    request_context: &'a RequestContext<'a>,
    subsystem_resolver: &'a DenoSubsystemResolver,
    system_resolver: &'a SystemResolver,
) -> Result<DenoOperation<'a>, DenoExecutionError> {
    // TODO: Remove unwrap() by changing the type of method_id
    let method = &system.methods[method_id.unwrap()];

    Ok(DenoOperation {
        method,
        field,
        request_context,
        subsystem_resolver,
        system_resolver,
    })
}

impl From<DenoExecutionError> for SubsystemResolutionError {
    fn from(e: DenoExecutionError) -> Self {
        match e {
            DenoExecutionError::Authorization => SubsystemResolutionError::Authorization,
            _ => SubsystemResolutionError::UserDisplayError(
                e.user_error_message()
                    .unwrap_or_else(|| "Internal server error".to_string()),
            ),
        }
    }
}
