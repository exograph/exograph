use crate::execution::{
    query_context::{QueryContext, QueryResponse},
    resolver::FieldResolver,
};
use anyhow::{anyhow, Result};
use async_graphql_parser::{
    types::{Field, OperationType},
    Positioned,
};

use payas_model::model::system::ModelSystem;
use serde_json::Value;

use super::operation_mapper::OperationResolver;

pub trait DataResolver {
    fn resolve(
        &self,
        field: &Positioned<Field>,
        operation_type: &OperationType,
        query_context: &QueryContext<'_>,
    ) -> Result<QueryResponse>;
}

impl FieldResolver<Value> for Value {
    fn resolve_field<'a>(
        &'a self,
        _query_context: &QueryContext<'_>,
        field: &Positioned<Field>,
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

impl DataResolver for ModelSystem {
    fn resolve(
        &self,
        field: &Positioned<Field>,
        operation_type: &OperationType,
        query_context: &QueryContext<'_>,
    ) -> Result<QueryResponse> {
        match operation_type {
            OperationType::Query => {
                let operation = self.queries.get_by_key(&field.node.name.node).unwrap();
                operation.execute(field, query_context)
            }
            OperationType::Mutation => {
                let operation = self.mutations.get_by_key(&field.node.name.node).unwrap();
                operation.execute(field, query_context)
            }
            OperationType::Subscription => {
                todo!()
            }
        }
    }
}
