use crate::graphql::{execution::system_context::SystemContext, execution_error::ExecutionError};
use async_trait::async_trait;

use payas_model::model::operation::{Mutation, MutationKind};
use payas_resolver_core::request_context::RequestContext;
use payas_resolver_core::validation::field::ValidatedField;

use crate::graphql::data::{data_operation::DataOperation, operation_resolver::OperationResolver};

use payas_resolver_database::{DatabaseMutation, DatabaseSystemContext};

use super::service_util::create_service_operation;

#[async_trait]
impl<'a> OperationResolver<'a> for Mutation {
    async fn resolve_operation(
        &'a self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<DataOperation<'a>, ExecutionError> {
        match &self.kind {
            MutationKind::Database { kind } => {
                let database_mutation = DatabaseMutation {
                    kind,
                    return_type: &self.return_type,
                };
                let database_system_context = DatabaseSystemContext {
                    system: &system_context.system,
                    database_executor: &system_context.database_executor,
                    resolve_operation_fn: system_context.resolve_operation_fn(),
                };
                database_mutation
                    .operation(field, &database_system_context, request_context)
                    .await
                    .map_err(ExecutionError::Database)
                    .map(DataOperation::Sql)
            }

            MutationKind::Service { method_id, .. } => {
                create_service_operation(&system_context.system, method_id, field, request_context)
            }
        }
    }
}
