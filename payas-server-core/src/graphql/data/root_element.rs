use async_graphql_parser::types::OperationType;
use payas_model::model::system::ModelSystem;

use crate::graphql::execution::system_context::SystemContext;
use crate::graphql::{
    execution::query_response::QueryResponse, execution_error::ExecutionError,
    request_context::RequestContext, validation::field::ValidatedField,
};

use super::operation_resolver::OperationResolver;

pub struct DataRootElement<'a> {
    pub system: &'a ModelSystem,
    pub operation_type: &'a OperationType,
}

impl<'a> DataRootElement<'a> {
    pub async fn resolve(
        &self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<QueryResponse, ExecutionError> {
        let name = &field.name;

        match self.operation_type {
            OperationType::Query => {
                let operation =
                    self.system.queries.get_by_key(name).ok_or_else(|| {
                        ExecutionError::Generic(format!("No such query {}", name))
                    })?;
                operation
                    .execute(field, system_context, request_context)
                    .await
            }
            OperationType::Mutation => {
                let operation =
                    self.system.mutations.get_by_key(name).ok_or_else(|| {
                        ExecutionError::Generic(format!("No such mutation {}", name))
                    })?;
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
