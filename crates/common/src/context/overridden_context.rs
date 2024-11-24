// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use elsa::sync::FrozenMap;
use serde_json::Value;

use crate::value::Val;

use super::{ContextExtractionError, RequestContext};

/// Represents a request context that has been overridden explicitly through
/// a call to `ExographPriv` with overriddent context that should last only for that call (including any nested calls).
/// In other words, once the call returns, the `base_context`
pub struct OverriddenContext<'a> {
    pub base_context: &'a RequestContext<'a>,
    context_override: Value,
    context_cache: FrozenMap<(String, String), Box<Option<Val>>>,
}

impl<'a> OverriddenContext<'a> {
    pub fn new(base_context: &'a RequestContext<'a>, context_override: Value) -> Self {
        Self {
            base_context,
            context_override,
            context_cache: FrozenMap::new(),
        }
    }

    pub async fn extract_context_field(
        &'a self,
        context_type_name: &str,
        source_annotation: &str,
        source_annotation_key: &Option<&str>,
        field_name: &str,
        coerce_value: &(impl Fn(Val) -> Result<Val, ContextExtractionError> + std::marker::Sync),
    ) -> Result<Option<&'a Val>, ContextExtractionError> {
        let cache_key = (context_type_name.to_owned(), field_name.to_owned());

        let cached_value: Option<&Option<Val>> = self.context_cache.get(&cache_key);

        match cached_value {
            Some(value) => Ok(value.as_ref()),
            None => {
                let overridden: Option<Val> = self
                    .context_override
                    .get(context_type_name)
                    .and_then(|value| value.get(field_name).map(|value| value.clone().into()));
                let coerced_value = overridden.map(coerce_value).transpose()?;

                match coerced_value {
                    Some(_) => Ok(self
                        .context_cache
                        .insert(cache_key, Box::new(coerced_value))
                        .as_ref()),
                    None => {
                        self.base_context
                            .extract_context_field(
                                context_type_name,
                                source_annotation,
                                source_annotation_key,
                                field_name,
                                coerce_value,
                            )
                            .await
                    }
                }
            }
        }
    }

    pub async fn ensure_transaction(&self) {
        self.base_context.ensure_transaction().await;
    }
}
