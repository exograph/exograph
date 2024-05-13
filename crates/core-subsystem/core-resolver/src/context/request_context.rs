// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_recursion::async_recursion;

use crate::{system_resolver::SystemResolver, value::Val};

use super::{
    context_extractor::BoxedContextExtractor, error::ContextExtractionError,
    overridden_context::OverriddenContext, request::Request,
    user_request_context::UserRequestContext,
};

use serde_json::Value;

pub enum RequestContext<'a> {
    // The original request context (before any overrides)
    User(UserRequestContext<'a>),

    // The recursive nature allows stacking overrides
    Overridden(OverriddenContext<'a>),
}

impl<'a> RequestContext<'a> {
    pub fn new(
        request: &'a (dyn Request + Send + Sync),
        parsed_contexts: Vec<BoxedContextExtractor<'a>>,
        system_resolver: &'a SystemResolver,
    ) -> Result<RequestContext<'a>, ContextExtractionError> {
        Ok(RequestContext::User(UserRequestContext::new(
            request,
            parsed_contexts,
            system_resolver,
        )?))
    }

    pub fn with_override(&'a self, context_override: Value) -> RequestContext<'a> {
        RequestContext::Overridden(OverriddenContext::new(self, context_override))
    }

    pub fn get_base_context(&self) -> &UserRequestContext {
        match &self {
            RequestContext::User(req) => req,
            RequestContext::Overridden(OverriddenContext { base_context, .. }) => {
                base_context.get_base_context()
            }
        }
    }

    #[async_recursion]
    pub async fn extract_context_field(
        &'a self,
        context_type_name: &str,
        source_annotation: &str,
        source_annotation_key: &Option<&str>,
        field_name: &str,
        coerce_value: &(impl Fn(Val) -> Result<Val, ContextExtractionError> + std::marker::Sync),
    ) -> Result<Option<&'a Val>, ContextExtractionError> {
        match self {
            RequestContext::User(user_request_context) => {
                user_request_context
                    .extract_context_field(
                        source_annotation,
                        source_annotation_key.unwrap_or(field_name),
                        coerce_value,
                        self,
                    )
                    .await
            }
            RequestContext::Overridden(overridden_context) => {
                overridden_context
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

    #[async_recursion]
    pub async fn ensure_transaction(&self) {
        match self {
            RequestContext::User(user_request_context) => {
                user_request_context.ensure_transaction().await;
            }
            RequestContext::Overridden(overridden_context) => {
                overridden_context.ensure_transaction().await;
            }
        }
    }
}
