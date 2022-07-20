use crate::{
    execution::{
        resolver::FieldResolver,
        system_context::{QueryResponse, SystemContext},
    },
    execution_error::ExecutionError,
    request_context::RequestContext,
    validation::field::ValidatedField,
};
use async_graphql_parser::types::OperationType;
use async_trait::async_trait;

use payas_model::model::system::ModelSystem;
use serde_json::Value;

use super::operation_mapper::OperationResolver;

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
impl FieldResolver<Value> for Value {
    async fn resolve_field<'a>(
        &'a self,
        field: &ValidatedField,
        _system_context: &'a SystemContext,
        _request_context: &'a RequestContext<'a>,
    ) -> Result<Value, ExecutionError> {
        let field_name = field.name.as_str();

        if let Value::Object(map) = self {
            map.get(field_name).cloned().ok_or_else(|| {
                ExecutionError::Generic(format!("No field named {} in Object", field_name))
            })
        } else {
            Err(ExecutionError::Generic(format!(
                "{} is not an Object and doesn't have any fields",
                field_name
            )))
        }
    }
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
