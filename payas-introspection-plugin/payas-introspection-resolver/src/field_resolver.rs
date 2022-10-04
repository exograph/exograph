use async_trait::async_trait;
use futures::StreamExt;

use payas_core_resolver::{
    introspection::definition::schema::Schema, request_context::RequestContext,
    validation::field::ValidatedField,
};

// TODO: This is duplicated from payas-core-resolver to avoid the orphan rule. Find a better solution.
#[async_trait]
pub trait FieldResolver<R, E>
where
    Self: std::fmt::Debug,
    R: Send + Sync,
    E: Send + Sync,
{
    // {
    //   name: ???
    // }
    // `field` is `name` and ??? is the return value
    async fn resolve_field<'e>(
        &'e self,
        field: &ValidatedField,
        schema: &Schema,
        request_context: &'e RequestContext<'e>,
    ) -> Result<R, E>;

    async fn resolve_fields(
        &self,
        fields: &[ValidatedField],
        schema: &Schema,
        request_context: &RequestContext<'_>,
    ) -> Result<Vec<(String, R)>, E> {
        futures::stream::iter(fields.iter())
            .then(|field| async {
                self.resolve_field(field, schema, request_context)
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