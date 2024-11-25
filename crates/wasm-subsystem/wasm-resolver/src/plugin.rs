// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{wasm_execution_error::WasmExecutionError, wasm_operation::WasmOperation};
use async_graphql_parser::types::{FieldDefinition, OperationType, TypeDefinition};
use async_trait::async_trait;
use common::context::RequestContext;
use core_plugin_interface::{
    core_model::mapped_arena::SerializableSlabIndex,
    core_resolver::{
        plugin::{SubsystemGraphQLResolver, SubsystemResolutionError},
        system_resolver::GraphQLSystemResolver,
        validation::field::ValidatedField,
        InterceptedOperation, QueryResponse,
    },
    interception::InterceptorIndex,
    interface::{SubsystemLoader, SubsystemLoadingError, SubsystemResolver},
    serializable_system::SerializableSubsystem,
    system_serializer::SystemSerializer,
};
use exo_env::Environment;
use exo_wasm::WasmExecutorPool;
use wasm_model::{module::ModuleMethod, subsystem::WasmSubsystem};

pub struct WasmSubsystemLoader {}

#[async_trait]
impl SubsystemLoader for WasmSubsystemLoader {
    fn id(&self) -> &'static str {
        "wasm"
    }

    async fn init(
        &mut self,
        serialized_subsystem: SerializableSubsystem,
        _env: &dyn Environment,
    ) -> Result<Box<SubsystemResolver>, SubsystemLoadingError> {
        let executor = WasmExecutorPool::default();

        let graphql = match serialized_subsystem.graphql {
            Some(graphql) => {
                let subsystem = WasmSubsystem::deserialize(graphql.0)?;

                Ok::<_, SubsystemLoadingError>(Some(Box::new(WasmSubsystemResolver {
                    id: self.id(),
                    subsystem,
                    executor,
                })
                    as Box<dyn SubsystemGraphQLResolver + Send + Sync>))
            }
            None => Ok(None),
        }?;

        Ok(Box::new(SubsystemResolver::new(graphql, None)))
    }
}

pub struct WasmSubsystemResolver {
    pub id: &'static str,
    pub subsystem: WasmSubsystem,
    pub executor: WasmExecutorPool,
}

#[async_trait]
impl SubsystemGraphQLResolver for WasmSubsystemResolver {
    fn id(&self) -> &'static str {
        self.id
    }

    async fn resolve<'a>(
        &'a self,
        field: &'a ValidatedField,
        operation_type: OperationType,
        request_context: &'a RequestContext<'a>,
        system_resolver: &'a GraphQLSystemResolver,
    ) -> Result<Option<QueryResponse>, SubsystemResolutionError> {
        // If the top-level operation is WASM, we can't be sure what the code will do, so we must
        // ensure a transaction.

        request_context.ensure_transaction().await;

        let operation_name = &field.name;

        let operation = match operation_type {
            OperationType::Query => {
                let query = self.subsystem.queries.get_by_key(operation_name);
                query.map(|query| {
                    create_wasm_operation(
                        &self.subsystem,
                        &query.method_id,
                        field,
                        request_context,
                        self,
                        system_resolver,
                    )
                })
            }
            OperationType::Mutation => {
                let mutation = self.subsystem.mutations.get_by_key(operation_name);
                mutation.map(|mutation| {
                    create_wasm_operation(
                        &self.subsystem,
                        &mutation.method_id,
                        field,
                        request_context,
                        self,
                        system_resolver,
                    )
                })
            }
            OperationType::Subscription => Some(Err(WasmExecutionError::Generic(
                "Subscriptions are not supported".to_string(),
            ))),
        };

        match operation {
            Some(Ok(operation)) => Ok(Some(operation.execute().await?)),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    async fn invoke_interceptor<'a>(
        &'a self,
        _interceptor_index: InterceptorIndex,
        _intercepted_operation: &'a InterceptedOperation<'a>,
        _request_context: &'a RequestContext<'a>,
        _system_resolver: &'a GraphQLSystemResolver,
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

pub(crate) fn create_wasm_operation<'a>(
    system: &'a WasmSubsystem,
    method_id: &Option<SerializableSlabIndex<ModuleMethod>>,
    field: &'a ValidatedField,
    request_context: &'a RequestContext<'a>,
    subsystem_resolver: &'a WasmSubsystemResolver,
    system_resolver: &'a GraphQLSystemResolver,
) -> Result<WasmOperation<'a>, WasmExecutionError> {
    // TODO: Remove unwrap() by changing the type of method_id
    let method = &system.methods[method_id.unwrap()];

    Ok(WasmOperation {
        method,
        field,
        request_context,
        subsystem_resolver,
        system_resolver,
    })
}

impl From<WasmExecutionError> for SubsystemResolutionError {
    fn from(e: WasmExecutionError) -> Self {
        match e {
            WasmExecutionError::Authorization => SubsystemResolutionError::Authorization,
            _ => SubsystemResolutionError::UserDisplayError(e.user_error_message()),
        }
    }
}
