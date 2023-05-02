// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_trait::async_trait;
use futures::StreamExt;

use crate::{context::RequestContext, validation::field::ValidatedField};

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
//     fn resolve_field(&self, field: &ValidatedField, system_resolver: &SystemContext, request_context: &request_context::RequestContext<'_>,) -> Value {
//         match self {
//             Some(td) => td.resolve_field(system_resolver, field),
//             None => Value::Null,
//         }
//     }
// }
