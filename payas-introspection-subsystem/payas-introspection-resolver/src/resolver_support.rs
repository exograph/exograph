use async_graphql_parser::Positioned;
use async_trait::async_trait;
use futures::StreamExt;
use payas_core_resolver::introspection::definition::schema::Schema;
use payas_core_resolver::plugin::SubsystemResolutionError;
use payas_core_resolver::request_context::RequestContext;
use serde_json::Value;

use payas_core_resolver::validation::field::ValidatedField;

use crate::field_resolver::FieldResolver;

#[async_trait]
pub(super) trait Resolver {
    async fn resolve_value<'e>(
        &self,
        fields: &'e [ValidatedField],
        schema: &Schema,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value, SubsystemResolutionError>;
}

#[async_trait]
impl<T> Resolver for Vec<T>
where
    T: Resolver + std::fmt::Debug + Send + Sync,
{
    async fn resolve_value<'e>(
        &self,
        fields: &'e [ValidatedField],
        schema: &Schema,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value, SubsystemResolutionError> {
        let resolved: Vec<_> = futures::stream::iter(self.iter())
            .then(|elem| elem.resolve_value(fields, schema, request_context))
            .collect()
            .await;

        let resolved: Result<Vec<Value>, SubsystemResolutionError> = resolved.into_iter().collect();

        Ok(Value::Array(resolved?))
    }
}

#[async_trait]
impl<T> Resolver for T
where
    T: FieldResolver<Value, SubsystemResolutionError> + std::fmt::Debug + Send + Sync,
{
    async fn resolve_value<'e>(
        &self,
        fields: &'e [ValidatedField],
        schema: &Schema,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value, SubsystemResolutionError> {
        Ok(Value::Object(FromIterator::from_iter(
            self.resolve_fields(fields, schema, request_context).await?,
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
        schema: &Schema,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value, SubsystemResolutionError> {
        self.node
            .resolve_value(fields, schema, request_context)
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
        schema: &Schema,
        request_context: &'e RequestContext<'e>,
    ) -> Result<Value, SubsystemResolutionError> {
        match self {
            Some(elem) => elem.resolve_value(fields, schema, request_context).await,
            None => Ok(Value::Null),
        }
    }
}
