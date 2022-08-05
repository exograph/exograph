use crate::graphql::{execution::system_context::SystemContext, execution_error::ExecutionError};
use async_trait::async_trait;

use payas_model::model::operation::{Interceptors, Mutation, MutationKind};
use payas_resolver_core::request_context::RequestContext;
use payas_resolver_core::validation::field::ValidatedField;

use crate::graphql::data::{
    operation_mapper::OperationResolverResult, operation_resolver::OperationResolver,
};

use payas_resolver_database::{DatabaseExecutionError, DatabaseMutation, DatabaseSystemContext};

use payas_resolver_deno::DenoOperation;

#[async_trait]
impl<'a> OperationResolver<'a> for Mutation {
    async fn resolve_operation(
        &'a self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<OperationResolverResult<'a>, ExecutionError> {
        match &self.kind {
            MutationKind::Database { kind } => {
                let database_mutation = DatabaseMutation {
                    kind,
                    return_type: &self.return_type,
                };
                let database_system_context = DatabaseSystemContext {
                    system: &system_context.system,
                    database_executor: &system_context.database_executor,
                    resolve: system_context.curried_resolve(),
                };
                database_mutation
                    .operation(field, &database_system_context, request_context)
                    .await
                    .map_err(|database_execution_error| match database_execution_error {
                        DatabaseExecutionError::Authorization => ExecutionError::Authorization,
                        DatabaseExecutionError::Generic(message) => {
                            ExecutionError::Generic(message)
                        }
                        e => ExecutionError::Database(e),
                    })
                    .map(OperationResolverResult::SQLOperation)
            }

            MutationKind::Service { method_id, .. } => Ok(OperationResolverResult::DenoOperation(
                DenoOperation(method_id.unwrap()),
            )),
        }
    }

    fn interceptors(&self) -> &Interceptors {
        &self.interceptors
    }

    fn name(&self) -> &str {
        &self.name
    }
}
