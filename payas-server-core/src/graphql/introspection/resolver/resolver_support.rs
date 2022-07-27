use async_graphql_parser::Positioned;
use async_trait::async_trait;
use futures::StreamExt;
use serde_json::Value;

use crate::{
    graphql::{
        execution::field_resolver::FieldResolver, execution_error::ExecutionError,
        validation::field::ValidatedField,
    },
    request_context::RequestContext,
    SystemContext,
};

#[async_trait]
pub(super) trait Resolver {
    async fn resolve_value<'e>(
        &self,
        fields: &'e [ValidatedField],
        system_context: &'e SystemContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value, ExecutionError>;
}

#[async_trait]
impl<T> Resolver for Vec<T>
where
    T: Resolver + std::fmt::Debug + Send + Sync,
{
    async fn resolve_value<'e>(
        &self,
        fields: &'e [ValidatedField],
        system_context: &'e SystemContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value, ExecutionError> {
        let resolved: Vec<_> = futures::stream::iter(self.iter())
            .then(|elem| elem.resolve_value(fields, system_context, request_context))
            .collect()
            .await;

        let resolved: Result<Vec<Value>, ExecutionError> = resolved.into_iter().collect();

        Ok(Value::Array(resolved?))
    }
}

#[async_trait]
impl<T> Resolver for T
where
    T: FieldResolver<Value, ExecutionError, SystemContext> + std::fmt::Debug + Send + Sync,
{
    async fn resolve_value<'e>(
        &self,
        fields: &'e [ValidatedField],
        system_context: &'e SystemContext,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value, ExecutionError> {
        Ok(Value::Object(FromIterator::from_iter(
            self.resolve_fields(fields, system_context, request_context)
                .await?,
        )))
    }
}

#[async_trait]
impl<T> Resolver for Positioned<T>
where
    T: Resolver + std::fmt::Debug + Send + Sync,
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
impl<T> Resolver for Option<&T>
where
    T: Resolver + std::fmt::Debug + Send + Sync,
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
