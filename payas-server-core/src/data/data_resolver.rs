use crate::{
    execution::{
        resolver::FieldResolver,
        system_context::{QueryResponse, SystemContext},
    },
    request_context::RequestContext,
    validation::field::ValidatedField,
};
use anyhow::Context;
use anyhow::{anyhow, Result};
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
    ) -> Result<QueryResponse>;
}

#[async_trait]
impl FieldResolver<Value> for Value {
    async fn resolve_field<'a>(
        &'a self,
        field: &ValidatedField,
        _system_context: &'a SystemContext,
        _request_context: &'a RequestContext<'a>,
    ) -> Result<Value> {
        let field_name = field.name.as_str();

        if let Value::Object(map) = self {
            map.get(field_name)
                .cloned()
                .ok_or_else(|| anyhow!("No field named {} in Object", field_name))
        } else {
            Err(anyhow!(
                "{} is not an Object and doesn't have any fields",
                field_name
            ))
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
    ) -> Result<QueryResponse> {
        let name = &field.name;

        match operation_type {
            OperationType::Query => {
                let operation = self
                    .queries
                    .get_by_key(name)
                    .with_context(|| format!("No such query {}", name))?;
                operation
                    .execute(field, system_context, request_context)
                    .await
            }
            OperationType::Mutation => {
                let operation = self
                    .mutations
                    .get_by_key(name)
                    .with_context(|| format!("No such mutation {}", name))?;
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
