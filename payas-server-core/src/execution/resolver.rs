use std::iter::FromIterator;

use anyhow::Result;
use async_graphql_parser::Positioned;
use async_trait::async_trait;
use futures::StreamExt;
use serde_json::Value;

use crate::validation::field::ValidatedField;

use super::query_context::QueryContext;

#[async_trait(?Send)]
pub trait Resolver<R> {
    async fn resolve_value<'e>(
        &self,
        query_context: &'e QueryContext<'e>,
        selection_set: &'e [ValidatedField],
    ) -> Result<R>;
}

#[async_trait(?Send)]
pub trait FieldResolver<R>
where
    Self: std::fmt::Debug,
{
    // {
    //   name: ???
    // }
    // `field` is `name` and ??? is the return value
    async fn resolve_field<'e>(
        &'e self,
        query_context: &'e QueryContext<'e>,
        field: &ValidatedField,
    ) -> Result<R>;

    async fn resolve_fields(
        &self,
        query_context: &QueryContext<'_>,
        fields: &[ValidatedField],
    ) -> Result<Vec<(String, R)>> {
        futures::stream::iter(fields.iter())
            .then(|field| async {
                self.resolve_field(query_context, field)
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
//     fn resolve_field(&self, query_context: &QueryContext<'_>, field: &Field) -> Value {
//         match self {
//             Some(td) => td.resolve_field(query_context, field),
//             None => Value::Null,
//         }
//     }
// }

#[async_trait(?Send)]
impl<T> Resolver<Value> for T
where
    T: FieldResolver<Value> + std::fmt::Debug,
{
    async fn resolve_value<'e>(
        &self,
        query_context: &'e QueryContext<'e>,
        selection_set: &'e [ValidatedField],
    ) -> Result<Value> {
        Ok(Value::Object(FromIterator::from_iter(
            self.resolve_fields(query_context, selection_set).await?,
        )))
    }
}

#[async_trait(?Send)]
impl<T> Resolver<Value> for Option<&T>
where
    T: Resolver<Value> + std::fmt::Debug,
{
    async fn resolve_value<'e>(
        &self,
        query_context: &'e QueryContext<'e>,
        selection_set: &'e [ValidatedField],
    ) -> Result<Value> {
        match self {
            Some(elem) => elem.resolve_value(query_context, selection_set).await,
            None => Ok(Value::Null),
        }
    }
}

#[async_trait(?Send)]
impl<T> Resolver<Value> for Positioned<T>
where
    T: Resolver<Value> + std::fmt::Debug,
{
    async fn resolve_value<'e>(
        &self,
        query_context: &'e QueryContext<'e>,
        selection_set: &'e [ValidatedField],
    ) -> Result<Value> {
        self.node.resolve_value(query_context, selection_set).await
    }
}

#[async_trait(?Send)]
impl<T> Resolver<Value> for Vec<T>
where
    T: Resolver<Value> + std::fmt::Debug,
{
    async fn resolve_value<'e>(
        &self,
        query_context: &'e QueryContext<'e>,
        selection_set: &'e [ValidatedField],
    ) -> Result<Value> {
        let resolved: Vec<_> = futures::stream::iter(self.iter())
            .then(|elem| elem.resolve_value(query_context, selection_set))
            .collect()
            .await;

        let resolved: Result<Vec<Value>> = resolved.into_iter().collect();

        Ok(Value::Array(resolved?))
    }
}
