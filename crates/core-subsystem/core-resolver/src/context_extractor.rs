// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use async_trait::async_trait;
use core_model::context_type::{ContextContainer, ContextSelection, ContextType};
use futures::StreamExt;

use crate::{context::RequestContext, value::Val};

/// Extract context objects from the request context.
#[async_trait]
pub trait ContextExtractor {
    fn context_type(&self, context_type_name: &str) -> &ContextType;

    /// Extract the context object.
    ///
    /// If the context type is defined as:
    ///
    /// ```exo
    /// context AuthContext {
    ///   id: Int
    ///   name: String
    ///   role: String
    /// }
    /// ```
    ///
    /// Then calling this with `context_name` set to `"AuthContext"` will return an object
    /// such as:
    ///
    /// ```json
    /// {
    ///   id: 1,
    ///   name: "John",
    ///   role: "admin",
    /// }
    /// ```
    async fn extract_context(
        &self,
        request_context: &RequestContext,
        context_type_name: &str,
    ) -> Option<Val> {
        let context_type = self.context_type(context_type_name);
        let field_values: HashMap<_, _> = futures::stream::iter(context_type.fields.iter())
            .then(|field| async {
                request_context
                    .extract_context_field(context_type_name, field)
                    .await
                    .map(|value| value.map(|value| (field.name.clone(), value.clone())))
            })
            .collect::<Vec<Result<_, _>>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .into_iter()
            .flatten()
            .collect();

        Some(Val::Object(field_values))
    }

    /// Extract the context object selection.
    ///
    /// This method is similar to `extract_context` but it allows to select a specific field from
    /// the context object. For example, consider the context type and the context object in the
    /// documentation of [`extract_context`](Self::extract_context). Calling this method with
    /// `context_selection` set to
    /// `AccessContextSelection::Select(AccessContextSelection("AuthContext"), "role")` will return
    /// the value `"admin"`.
    async fn extract_context_selection<'a>(
        &self,
        request_context: &'a RequestContext<'a>,
        context_selection: &ContextSelection,
    ) -> Option<&'a Val> {
        let context_type = self.context_type(&context_selection.context_name);
        let context_field = context_type
            .fields
            .iter()
            .find(|f| f.name == context_selection.path.0)?;

        request_context
            .extract_context_field(&context_selection.context_name, context_field)
            .await
            .unwrap()
    }
}

#[async_trait]
impl<T: ContextContainer + std::marker::Sync> ContextExtractor for T {
    fn context_type(&self, context_type_name: &str) -> &ContextType {
        let contexts = self.contexts();
        contexts.get_by_key(context_type_name).unwrap()
    }
}
