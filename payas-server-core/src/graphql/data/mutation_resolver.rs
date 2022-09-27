use crate::graphql::{execution::system_context::SystemContext, execution_error::ExecutionError};
use async_trait::async_trait;

use payas_database_model::operation::DatabaseMutation;
use payas_deno_model::operation::ServiceMutation;
use payas_resolver_core::request_context::RequestContext;
use payas_resolver_core::validation::field::ValidatedField;

use crate::graphql::data::data_operation::DataOperation;

use payas_resolver_database::{database_mutation::operation, DatabaseSystemContext};

use super::{
    operation_resolver::{DatabaseOperationResolver, ServiceOperationResolver},
    service_util::create_service_operation,
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
impl<'a> ServiceOperationResolver<'a> for ServiceMutation {
    async fn resolve_operation(
        &'a self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<DataOperation<'a>, ExecutionError> {
        create_service_operation(
            &system_context.system.service_subsystem,
            &self.method_id,
            field,
            request_context,
        )
    }
}
