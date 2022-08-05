use async_graphql_parser::types::OperationType;

use payas_resolver_core::validation::field::ValidatedField;
use payas_resolver_core::{request_context::RequestContext, QueryResponse};

use crate::graphql::execution::system_context::SystemContext;
use crate::graphql::execution_error::ExecutionError;

use super::operation_resolver::OperationResolver;

pub struct DataRootElement<'a> {
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
        let system = &system_context.system;

        match self.operation_type {
            OperationType::Query => {
                let query = system
                    .queries
                    .get_by_key(name)
                    .ok_or_else(|| ExecutionError::Generic(format!("No such query {}", name)))?;
                query.execute(field, system_context, request_context).await
            }
            OperationType::Mutation => {
                let mutation = system
                    .mutations
                    .get_by_key(name)
                    .ok_or_else(|| ExecutionError::Generic(format!("No such mutation {}", name)))?;
                mutation
                    .execute(field, system_context, request_context)
                    .await
            }
            OperationType::Subscription => {
                todo!()
            }
        }
    }
}
