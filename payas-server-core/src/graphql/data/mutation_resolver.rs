use crate::graphql::{execution::system_context::SystemContext, execution_error::ExecutionError};
use async_trait::async_trait;

use payas_database_model::operation::DatabaseMutation;
use payas_deno_model::operation::DenoMutation;
use payas_resolver_core::request_context::RequestContext;
use payas_resolver_core::validation::field::ValidatedField;
use payas_wasm_model::operation::WasmMutation;

use crate::graphql::data::data_operation::DataOperation;

use payas_resolver_database::{database_mutation::operation, DatabaseSystemContext};

use super::{
    operation_resolver::{DatabaseOperationResolver, DenoOperationResolver, WasmOperationResolver},
    service_util::{create_deno_operation, create_wasm_operation},
};

#[async_trait]
impl<'a> DatabaseOperationResolver<'a> for DatabaseMutation {
    async fn resolve_operation(
        &'a self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<DataOperation<'a>, ExecutionError> {
        let database_system_context = DatabaseSystemContext {
            system: &system_context.system.database_subsystem,
            database_executor: &system_context.database_executor,
            resolve_operation_fn: system_context.resolve_operation_fn(),
        };

        operation(self, field, &database_system_context, request_context)
            .await
            .map_err(ExecutionError::Database)
            .map(DataOperation::Sql)
    }
}

#[async_trait]
impl<'a> DenoOperationResolver<'a> for DenoMutation {
    async fn resolve_operation(
        &'a self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<DataOperation<'a>, ExecutionError> {
        create_deno_operation(
            &system_context.system.deno_subsystem,
            &self.method_id,
            field,
            request_context,
        )
    }
}

#[async_trait]
impl<'a> WasmOperationResolver<'a> for WasmMutation {
    async fn resolve_operation(
        &'a self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<DataOperation<'a>, ExecutionError> {
        create_wasm_operation(
            &system_context.system.wasm_subsystem,
            &self.method_id,
            field,
            request_context,
        )
    }
}
