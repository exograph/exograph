use crate::graphql::{execution::system_context::SystemContext, execution_error::ExecutionError};

use async_trait::async_trait;
use payas_database_model::operation::DatabaseQuery;
use payas_resolver_core::request_context::RequestContext;
use payas_resolver_core::validation::field::ValidatedField;

use payas_deno_model::operation::DenoQuery;
use payas_resolver_database::{database_query::compute_select, DatabaseSystemContext};
use payas_sql::{AbstractOperation, AbstractPredicate};
use payas_wasm_model::operation::WasmQuery;

use super::{
    data_operation::DataOperation,
    operation_resolver::{DatabaseOperationResolver, DenoOperationResolver, WasmOperationResolver},
    service_util::{create_deno_operation, create_wasm_operation},
};

#[async_trait]
impl<'a> DatabaseOperationResolver<'a> for DatabaseQuery {
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
        let operation = compute_select(
            self,
            field,
            AbstractPredicate::True,
            &database_system_context,
            request_context,
        )
        .await
        .map_err(ExecutionError::Database)?;

        Ok(DataOperation::Sql(AbstractOperation::Select(operation)))
    }
}

#[async_trait]
impl<'a> DenoOperationResolver<'a> for DenoQuery {
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
impl<'a> WasmOperationResolver<'a> for WasmQuery {
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
