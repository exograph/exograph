// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::sync::Arc;
use tokio::sync::Mutex;

use async_recursion::async_recursion;
use exo_env::Environment;

use crate::http::{RequestHead, RequestPayload, ResponsePayload};
use crate::router::PlainRequestPayload;
use crate::{router::Router, value::Val};
use exo_sql::TransactionHolder;

use super::JwtAuthenticator;
use super::{
    context_extractor::BoxedContextExtractor, error::ContextExtractionError,
    overridden_context::OverriddenContext, user_request_context::UserRequestContext,
};

use serde_json::Value;

pub struct RequestContext<'a> {
    core: CoreRequestContext<'a>,
    pub system_context: SystemRequestContext<'a>,
}

#[derive(Clone)]
pub struct SystemRequestContext<'a> {
    pub env: &'a dyn Environment,
    pub jwt_authenticator: &'a Option<JwtAuthenticator>,
    pub system_router: &'a dyn for<'request> Router<PlainRequestPayload<'request>>,
    pub transaction_holder: Arc<Mutex<TransactionHolder>>,
}

impl<'a> RequestContext<'a> {
    pub fn new(
        request: &'a (dyn RequestPayload + Send + Sync),
        parsed_contexts: Vec<BoxedContextExtractor<'a>>,
        system_router: &'a dyn for<'request> Router<PlainRequestPayload<'request>>,
        jwt_authenticator: &'a Option<JwtAuthenticator>,
        env: &'a dyn Environment,
    ) -> RequestContext<'a> {
        Self {
            core: CoreRequestContext::new(request, parsed_contexts),
            system_context: SystemRequestContext {
                env,
                jwt_authenticator,
                system_router,
                transaction_holder: Arc::new(Mutex::new(TransactionHolder::new())),
            },
        }
    }

    pub fn is_internal(&self) -> bool {
        matches!(self.core, CoreRequestContext::InternalRequest(..))
    }

    pub async fn extract_context_field(
        &'a self,
        context_type_name: &str,
        source_annotation: &str,
        source_annotation_key: &Option<&str>,
        field_name: &str,
        coerce_value: &(impl Fn(Val) -> Result<Val, ContextExtractionError> + std::marker::Sync),
    ) -> Result<Option<&'a Val>, ContextExtractionError> {
        self.core
            .extract_context_field(
                context_type_name,
                source_annotation,
                source_annotation_key,
                field_name,
                coerce_value,
                self,
            )
            .await
    }

    pub async fn ensure_transaction(&self) {
        self.system_context
            .transaction_holder
            .as_ref()
            .lock()
            .await
            .ensure_transaction();
    }

    pub async fn finalize_transaction(&self, commit: bool) -> Result<(), tokio_postgres::Error> {
        // Do not finalize internal requests (currently made through the QueryExtractor)
        if self.is_internal() {
            return Ok(());
        }

        self.system_context
            .transaction_holder
            .as_ref()
            .lock()
            .await
            .finalize(commit)
            .await
    }

    pub fn with_override(&'a self, context_override: Value) -> RequestContext<'a> {
        Self {
            core: self.core.with_override(context_override),
            system_context: self.system_context.clone(),
        }
    }

    pub fn with_request(
        &'a self,
        request: &'a (dyn RequestPayload + Send + Sync),
    ) -> RequestContext<'a> {
        Self {
            core: self.core.with_request(request),
            system_context: self.system_context.clone(),
        }
    }

    pub fn get_base_context(&self) -> &UserRequestContext<'a> {
        self.core.get_base_context()
    }

    pub async fn route(&self, request: &'a RequestContext<'a>) -> Option<ResponsePayload> {
        self.system_context
            .system_router
            .route(&PlainRequestPayload::internal(request))
            .await
    }

    /// Returns true if the request has authentication info present
    ///
    /// Helps identify if the request is unauthenticated or unauthorized (and send the appropriate status code)
    pub fn is_authentication_info_present(&self) -> bool {
        match self.system_context.jwt_authenticator {
            Some(authenticator) => authenticator
                .extract_jwt_token(self.get_head())
                .ok()
                .flatten()
                .is_some(),
            None => false,
        }
    }
}

impl<'a> RequestPayload for RequestContext<'a> {
    fn get_head(&self) -> &(dyn RequestHead + Send + Sync) {
        self.core.get_head()
    }

    fn take_body(&self) -> Value {
        self.core.take_body()
    }
}

pub(super) enum CoreRequestContext<'a> {
    // The original request context (before any overrides)
    User(UserRequestContext<'a>),

    // The recursive nature allows stacking overrides
    Overridden(OverriddenContext<'a>),

    InternalRequest(
        &'a CoreRequestContext<'a>,
        &'a (dyn RequestPayload + Send + Sync),
    ),
}

impl<'a> CoreRequestContext<'a> {
    pub fn new(
        request: &'a (dyn RequestPayload + Send + Sync),
        parsed_contexts: Vec<BoxedContextExtractor<'a>>,
    ) -> CoreRequestContext<'a> {
        Self::User(UserRequestContext::new(request, parsed_contexts))
    }

    pub fn with_request(
        &'a self,
        request: &'a (dyn RequestPayload + Send + Sync),
    ) -> CoreRequestContext<'a> {
        Self::InternalRequest(self, request)
    }

    pub fn with_override(&'a self, context_override: Value) -> CoreRequestContext<'a> {
        Self::Overridden(OverriddenContext::new(self, context_override))
    }

    pub fn get_base_context(&self) -> &UserRequestContext<'a> {
        match &self {
            Self::User(req) => req,
            Self::Overridden(OverriddenContext { base_context, .. }) => {
                base_context.get_base_context()
            }
            Self::InternalRequest(base_context, ..) => base_context.get_base_context(),
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
        request_context: &'a RequestContext<'a>,
    ) -> Result<Option<&'a Val>, ContextExtractionError> {
        match self {
            Self::User(user_request_context) => {
                user_request_context
                    .extract_context_field(
                        source_annotation,
                        source_annotation_key.unwrap_or(field_name),
                        coerce_value,
                        request_context,
                    )
                    .await
            }
            Self::Overridden(overridden_context) => {
                overridden_context
                    .extract_context_field(
                        context_type_name,
                        source_annotation,
                        source_annotation_key,
                        field_name,
                        coerce_value,
                        request_context,
                    )
                    .await
            }
            Self::InternalRequest(base_context, ..) => {
                base_context
                    .extract_context_field(
                        context_type_name,
                        source_annotation,
                        source_annotation_key,
                        field_name,
                        coerce_value,
                        request_context,
                    )
                    .await
            }
        }
    }
}

impl<'a> RequestPayload for CoreRequestContext<'a> {
    fn get_head(&self) -> &(dyn RequestHead + Send + Sync) {
        match self {
            Self::InternalRequest(_, request) => request.get_head(),
            _ => self.get_base_context().get_head(),
        }
    }

    fn take_body(&self) -> Value {
        match self {
            Self::InternalRequest(_, request) => request.take_body(),
            _ => self.get_base_context().get_request().take_body(),
        }
    }
}
