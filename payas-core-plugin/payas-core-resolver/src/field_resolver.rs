use async_trait::async_trait;
use futures::StreamExt;

use crate::{request_context::RequestContext, validation::field::ValidatedField};

#[async_trait]
pub trait FieldResolver<R, E, C>
where
    Self: std::fmt::Debug,
    R: Send + Sync, // Response
    E: Send + Sync, // Error
    C: Send + Sync,
{
    // {
    //   name: ???
    // }
    // `field` is `name` and ??? is the return value
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        resolution_context: &'e C,
        request_context: &'e RequestContext<'e>,
    ) -> Result<R, E>;

    async fn resolve_fields(
        &self,
        fields: &[ValidatedField],
        resolution_context: &C,
        request_context: &RequestContext<'_>,
    ) -> Result<Vec<(String, R)>, E> {
        futures::stream::iter(fields.iter())
            .then(|field| async {
                self.resolve_field(field, resolution_context, request_context)
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
