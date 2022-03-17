use crate::execution::{
    query_context::{QueryContext, QueryResponse},
    resolver::FieldResolver,
};
use anyhow::Context;
use anyhow::{anyhow, Result};
use async_graphql_parser::{
    types::{Field, OperationType},
    Positioned,
};
use async_trait::async_trait;

use payas_model::model::system::ModelSystem;
use serde_json::Value;

use super::operation_mapper::OperationResolver;

#[async_trait(?Send)]
pub trait DataResolver {
    async fn resolve<'e>(
        &self,
        field: &'e Positioned<Field>,
        operation_type: &'e OperationType,
        query_context: &'e QueryContext<'e>,
    ) -> Result<QueryResponse>;
}

#[async_trait(?Send)]
impl FieldResolver<Value> for Value {
    async fn resolve_field<'a>(
        &'a self,
        _query_context: &'a QueryContext<'a>,
        field: &'a Positioned<Field>,
    ) -> Result<Value> {
        let field_name = field.node.name.node.as_str();

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

#[async_trait(?Send)]
impl DataResolver for ModelSystem {
    async fn resolve<'e>(
        &self,
        field: &'e Positioned<Field>,
        operation_type: &'e OperationType,
        query_context: &'e QueryContext<'e>,
    ) -> Result<QueryResponse> {
        let name = &field.node.name.node;

        match operation_type {
            OperationType::Query => {
                let operation = self
                    .queries
                    .get_by_key(name)
                    .with_context(|| format!("No such query {}", name))?;
                operation.execute(field, query_context).await
            }
            OperationType::Mutation => {
                let operation = self
                    .mutations
                    .get_by_key(name)
                    .with_context(|| format!("No such mutation {}", name))?;
                operation.execute(field, query_context).await
            }
            OperationType::Subscription => {
                todo!()
            }
        }
    }
}
