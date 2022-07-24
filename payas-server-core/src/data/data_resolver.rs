use crate::execution::query_response::QueryResponse;
use crate::{
    execution::system_context::SystemContext, execution_error::ExecutionError,
    request_context::RequestContext, validation::field::ValidatedField,
};
use async_graphql_parser::types::OperationType;
use async_trait::async_trait;

use payas_model::model::system::ModelSystem;

use crate::resolver::OperationResolver;

#[async_trait]
pub trait DataResolver {
    async fn resolve<'e>(
        &self,
        field: &'e ValidatedField,
        operation_type: &'e OperationType,
        system_context: &'e SystemContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<QueryResponse, ExecutionError>;
}

#[async_trait]
impl DataResolver for ModelSystem {
    async fn resolve<'e>(
        &self,
        field: &'e ValidatedField,
        operation_type: &'e OperationType,
        system_context: &'e SystemContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<QueryResponse, ExecutionError> {
        let name = &field.name;

        match operation_type {
            OperationType::Query => {
                let operation = self
                    .queries
                    .get_by_key(name)
                    .ok_or_else(|| ExecutionError::Generic(format!("No such query {}", name)))?;
                operation
                    .execute(field, system_context, request_context)
                    .await
            }
            OperationType::Mutation => {
                let operation = self
                    .mutations
                    .get_by_key(name)
                    .ok_or_else(|| ExecutionError::Generic(format!("No such mutation {}", name)))?;
                operation
                    .execute(field, system_context, request_context)
                    .await
            }
            OperationType::Subscription => {
                todo!()
            }
        }
    }
}
