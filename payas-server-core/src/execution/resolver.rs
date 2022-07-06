use std::iter::FromIterator;

use anyhow::Result;
use async_graphql_parser::Positioned;
use async_trait::async_trait;
use futures::StreamExt;
use serde_json::Value;

use crate::{
    request_context::{self, RequestContext},
    validation::field::ValidatedField,
};

use super::operations_context::OperationsContext;

#[async_trait]
pub trait Resolver<R> {
    async fn resolve_value<'e>(
        &self,
        fields: &'e [ValidatedField],
        operations_context: &'e OperationsContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<R>;
}

#[async_trait]
pub trait FieldResolver<R>
where
    Self: std::fmt::Debug,
    R: Send + Sync,
{
    // {
    //   name: ???
    // }
    // `field` is `name` and ??? is the return value
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        operations_context: &'e OperationsContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<R>;

    async fn resolve_fields(
        &self,
        fields: &[ValidatedField],
        operations_context: &OperationsContext,
        request_context: &request_context::RequestContext<'_>,
    ) -> Result<Vec<(String, R)>> {
        futures::stream::iter(fields.iter())
            .then(|field| async {
                self.resolve_field(field, operations_context, request_context)
                    .await
                    .map(|value| (field.output_name(), value))
            })
            .collect::<Vec<Result<_>>>()
            .await
            .into_iter()
            .collect()
    }
}

#[derive(Debug)]
pub enum GraphQLExecutionError {
    InvalidField(String, &'static str), // (field name, container type)
    Authorization,
}

impl std::error::Error for GraphQLExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl std::fmt::Display for GraphQLExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphQLExecutionError::InvalidField(field_name, container_name) => {
                write!(f, "Invalid field {} for {}", field_name, container_name)
            }
            GraphQLExecutionError::Authorization => {
                write!(f, "Not authorized")
            }
        }
    }
}

// This might work after https://github.com/rust-lang/rfcs/blob/master/text/1210-impl-specialization.md
// impl<T> FieldResolver<Value> for Option<&T>
// where
//     T: FieldResolver<Value>,
// {
//     fn resolve_field(&self, operations_context: &QueryContext<'_>, field: &Field) -> Value {
//         match self {
//             Some(td) => td.resolve_field(operations_context, field),
//             None => Value::Null,
//         }
//     }
// }

#[async_trait]
impl<T> Resolver<Value> for T
where
    T: FieldResolver<Value> + std::fmt::Debug + Send + Sync,
{
    async fn resolve_value<'e>(
        &self,
        fields: &'e [ValidatedField],
        operations_context: &'e OperationsContext,
        request_context: &'e request_context::RequestContext<'e>,
    ) -> Result<Value> {
        Ok(Value::Object(FromIterator::from_iter(
            self.resolve_fields(fields, operations_context, request_context)
                .await?,
        )))
    }
}

#[async_trait]
impl<T> Resolver<Value> for Option<&T>
where
    T: Resolver<Value> + std::fmt::Debug + Send + Sync,
{
    async fn resolve_value<'e>(
        &self,
        fields: &'e [ValidatedField],
        operations_context: &'e OperationsContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value> {
        match self {
            Some(elem) => {
                elem.resolve_value(fields, operations_context, request_context)
                    .await
            }
            None => Ok(Value::Null),
        }
    }
}

#[async_trait]
impl<T> Resolver<Value> for Positioned<T>
where
    T: Resolver<Value> + std::fmt::Debug + Send + Sync,
{
    async fn resolve_value<'e>(
        &self,
        fields: &'e [ValidatedField],
        operations_context: &'e OperationsContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value> {
        self.node
            .resolve_value(fields, operations_context, request_context)
            .await
    }
}

#[async_trait]
impl<T> Resolver<Value> for Vec<T>
where
    T: Resolver<Value> + std::fmt::Debug + Send + Sync,
{
    async fn resolve_value<'e>(
        &self,
        fields: &'e [ValidatedField],
        operations_context: &'e OperationsContext,
        request_context: &'e request_context::RequestContext<'e>,
    ) -> Result<Value> {
        let resolved: Vec<_> = futures::stream::iter(self.iter())
            .then(|elem| elem.resolve_value(fields, operations_context, request_context))
            .collect()
            .await;

        let resolved: Result<Vec<Value>> = resolved.into_iter().collect();

        Ok(Value::Array(resolved?))
    }
}
