use payas_resolver_core::QueryResponse;
use payas_resolver_database::DatabaseSystemContext;

use payas_resolver_wasm::{WasmOperation, WasmSystemContext};
use payas_sql::AbstractOperation;

use crate::graphql::{execution::system_context::SystemContext, execution_error::ExecutionError};

use payas_resolver_deno::{DenoOperation, DenoSystemContext};

#[allow(clippy::large_enum_variant)]
pub enum DataOperation<'a> {
    Sql(AbstractOperation<'a>),
    Deno(DenoOperation<'a>),
    Wasm(WasmOperation<'a>),
}

impl<'a> DataOperation<'a> {
    pub async fn execute(
        &self,
        system_context: &'a SystemContext,
    ) -> Result<QueryResponse, ExecutionError> {
        let resolve_operation_fn = system_context.resolve_operation_fn();
        match self {
            DataOperation::Sql(abstract_operation) => {
                let database_system_context = DatabaseSystemContext {
                    system: &system_context.system,
                    database_executor: &system_context.database_executor,
                    resolve_operation_fn,
                };

                payas_resolver_database::resolve_operation(
                    abstract_operation,
                    database_system_context,
                )
                .await
                .map_err(ExecutionError::Database)
            }

            DataOperation::Deno(operation) => {
                let deno_system_context = DenoSystemContext {
                    system: &system_context.system,
                    deno_execution_pool: &system_context.deno_execution_pool,
                    resolve_operation_fn,
                };

                operation
                    .execute(&deno_system_context)
                    .await
                    .map_err(ExecutionError::Deno)
            }

            DataOperation::Wasm(operation) => {
                let wasm_system_context = WasmSystemContext {
                    system: &system_context.system,
                    executor_pool: &system_context.wasm_execution_pool,
                    resolve_operation_fn,
                };

                operation
                    .execute(&wasm_system_context)
                    .await
                    .map_err(ExecutionError::Wasm)
            }
        }
    }
}
