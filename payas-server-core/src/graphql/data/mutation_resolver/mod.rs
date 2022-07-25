use crate::graphql::{
    database::database_mutation::DatabaseMutation, execution::system_context::SystemContext,
    execution_error::ExecutionError, request_context::RequestContext,
    validation::field::ValidatedField,
};
use async_trait::async_trait;

use payas_model::model::operation::{Interceptors, Mutation, MutationKind};

use crate::graphql::data::{
    operation_mapper::{DenoOperation, OperationResolverResult},
    operation_resolver::OperationResolver,
};

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
                database_mutation
                    .operation(field, system_context, request_context)
                    .await
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
