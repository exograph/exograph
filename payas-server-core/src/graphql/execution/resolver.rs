use async_trait::async_trait;
use futures::StreamExt;

use crate::graphql::{
    execution_error::ExecutionError, request_context::RequestContext,
    validation::field::ValidatedField,
};

use super::system_context::SystemContext;

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
        request_context: &RequestContext<'_>,
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
//     fn resolve_field(&self, field: &ValidatedField, system_context: &SystemContext, request_context: &request_context::RequestContext<'_>,) -> Value {
//         match self {
//             Some(td) => td.resolve_field(system_context, field),
//             None => Value::Null,
//         }
//     }
// }
