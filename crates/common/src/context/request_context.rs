// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::sync::Arc;

use async_recursion::async_recursion;
use exo_env::Environment;

use crate::http::{RequestHead, RequestPayload};
use crate::router::PlainRequestPayload;
use crate::{router::Router, value::Val};

use super::JwtAuthenticator;
use super::{
    context_extractor::BoxedContextExtractor, error::ContextExtractionError,
    overridden_context::OverriddenContext, user_request_context::UserRequestContext,
};

use serde_json::Value;

pub enum RequestContext<'a> {
    // The original request context (before any overrides)
    User(UserRequestContext<'a>),

    // The recursive nature allows stacking overrides
    Overridden(OverriddenContext<'a>),

    OverriddenRequest(
        &'a RequestContext<'a>,
        &'a (dyn RequestPayload + Send + Sync),
    ),
}

impl<'a> RequestContext<'a> {
    pub fn new(
        request: &'a (dyn RequestPayload + Send + Sync),
        parsed_contexts: Vec<BoxedContextExtractor<'a>>,
        system_router: &'a (dyn for<'request> Router<PlainRequestPayload<'request>>),
        jwt_authenticator: Arc<Option<JwtAuthenticator>>,
        env: Arc<dyn Environment>,
    ) -> RequestContext<'a> {
        RequestContext::User(UserRequestContext::new(
            request,
            parsed_contexts,
            system_router,
            jwt_authenticator,
            env,
        ))
    }

    pub fn with_request(
        &'a self,
        request: &'a (dyn RequestPayload + Send + Sync),
    ) -> RequestContext<'a> {
        RequestContext::OverriddenRequest(self, request)
    }

    pub fn with_override(&'a self, context_override: Value) -> RequestContext<'a> {
        RequestContext::Overridden(OverriddenContext::new(self, context_override))
    }

    pub fn get_base_context(&self) -> &UserRequestContext<'a> {
        match &self {
            RequestContext::User(req) => req,
            RequestContext::Overridden(OverriddenContext { base_context, .. }) => {
                base_context.get_base_context()
            }
            RequestContext::OverriddenRequest(base_context, ..) => base_context.get_base_context(),
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
            RequestContext::OverriddenRequest(base_context, ..) => {
                base_context
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
            RequestContext::OverriddenRequest(base_context, ..) => {
                base_context.ensure_transaction().await;
            }
        }
    }

    #[async_recursion]
    pub async fn finalize_transaction(&self, commit: bool) -> Result<(), tokio_postgres::Error> {
        match self {
            // Do not finalize internal requests (currently made through the QueryExtractor)
            RequestContext::OverriddenRequest(..) => Ok(()),
            _ => self.get_base_context().finalize_transaction(commit).await,
        }
    }
}

impl<'a> RequestPayload for RequestContext<'a> {
    fn get_head(&self) -> &(dyn RequestHead + Send + Sync) {
        match self {
            RequestContext::OverriddenRequest(_, request) => request.get_head(),
            _ => self.get_base_context().get_head(),
        }
    }

    fn take_body(&self) -> Value {
        match self {
            RequestContext::OverriddenRequest(_, request) => request.take_body(),
            _ => self.get_base_context().get_request().take_body(),
        }
    }
}
