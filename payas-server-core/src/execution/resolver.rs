use std::iter::FromIterator;

use async_graphql_parser::Positioned;
use async_trait::async_trait;
use futures::StreamExt;
use serde_json::Value;

use crate::{
    execution_error::ExecutionError,
    request_context::{self, RequestContext},
    validation::field::ValidatedField,
};

use super::system_context::SystemContext;

#[async_trait]
pub trait Resolver<R> {
    async fn resolve_value<'e>(
        &self,
        fields: &'e [ValidatedField],
        system_context: &'e SystemContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<R, ExecutionError>;
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
        system_context: &'e SystemContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<R, ExecutionError>;

    async fn resolve_fields(
        &self,
        fields: &[ValidatedField],
        system_context: &SystemContext,
        request_context: &request_context::RequestContext<'_>,
    ) -> Result<Vec<(String, R)>, ExecutionError> {
        futures::stream::iter(fields.iter())
            .then(|field| async {
                self.resolve_field(field, system_context, request_context)
                    .await
                    .map(|value| (field.output_name(), value))
            })
            .collect::<Vec<Result<_, _>>>()
            .await
            .into_iter()
            .collect()
    }
}

// This might work after https://github.com/rust-lang/rfcs/blob/master/text/1210-impl-specialization.md
// impl<T> FieldResolver<Value> for Option<&T>
// where
//     T: FieldResolver<Value>,
// {
//     fn resolve_field(&self, system_context: &QueryContext<'_>, field: &Field) -> Value {
//         match self {
//             Some(td) => td.resolve_field(system_context, field),
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
        system_context: &'e SystemContext,
        request_context: &'e request_context::RequestContext<'e>,
    ) -> Result<Value, ExecutionError> {
        Ok(Value::Object(FromIterator::from_iter(
            self.resolve_fields(fields, system_context, request_context)
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
        system_context: &'e SystemContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value, ExecutionError> {
        match self {
            Some(elem) => {
                elem.resolve_value(fields, system_context, request_context)
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
        system_context: &'e SystemContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value, ExecutionError> {
        self.node
            .resolve_value(fields, system_context, request_context)
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
        system_context: &'e SystemContext,
        request_context: &'e request_context::RequestContext<'e>,
    ) -> Result<Value, ExecutionError> {
        let resolved: Vec<_> = futures::stream::iter(self.iter())
            .then(|elem| elem.resolve_value(fields, system_context, request_context))
            .collect()
            .await;

        let resolved: Result<Vec<Value>, ExecutionError> = resolved.into_iter().collect();

        Ok(Value::Array(resolved?))
    }
}
