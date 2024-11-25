// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::{
    context::RequestContext,
    http::{Headers, RequestHead, RequestPayload, ResponseBody, ResponsePayload},
};
use async_trait::async_trait;
use http::StatusCode;

#[async_trait]
pub trait Router<RQ: RequestPayload + Send + Sync + ?Sized>: Sync {
    async fn route(&self, request_context: &RQ) -> Option<ResponsePayload>;
}

#[async_trait]
impl<Rtr: Sync + ?Sized + Router<RQ>, RQ: RequestPayload + Send + Sync> Router<RQ> for Box<Rtr> {
    async fn route(&self, request_context: &RQ) -> Option<ResponsePayload> {
        (**self).route(request_context).await
    }
}

pub enum PlainRequestPayload<'a> {
    External(Box<dyn RequestPayload + Send + Sync>),
    Internal(&'a RequestContext<'a>),
    // request: Box<dyn RequestPayload + Send + Sync>,
}

impl<'a> PlainRequestPayload<'a> {
    pub fn external(request: Box<dyn RequestPayload + Send + Sync>) -> Self {
        Self::External(request)
    }

    pub fn internal(request: &'a RequestContext<'a>) -> Self {
        Self::Internal(request)
    }
}

impl RequestPayload for PlainRequestPayload<'_> {
    fn get_head(&self) -> &(dyn RequestHead + Send + Sync) {
        match self {
            Self::External(request) => request.get_head(),
            Self::Internal(request) => request.get_head(),
        }
    }

    fn take_body(&self) -> serde_json::Value {
        match self {
            Self::External(request) => request.take_body(),
            Self::Internal(request) => request.take_body(),
        }
    }
}

pub struct CompositeRouter<Rtr> {
    routers: Vec<Rtr>,
}

impl<Rtr> CompositeRouter<Rtr> {
    pub fn new(routers: Vec<Rtr>) -> Self {
        Self { routers }
    }
}

#[async_trait::async_trait]
impl<RQ: RequestPayload + Send + Sync, Rtr: Router<RQ>> Router<RQ> for CompositeRouter<Rtr> {
    async fn route(&self, request_context: &RQ) -> Option<ResponsePayload> {
        for router in self.routers.iter() {
            if let Some(response) = router.route(request_context).await {
                return Some(response);
            }
        }

        Some(ResponsePayload {
            body: ResponseBody::None,
            headers: Headers::new(),
            status_code: StatusCode::NOT_FOUND,
        })
    }
}
