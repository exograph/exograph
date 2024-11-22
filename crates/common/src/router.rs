// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::http::{Headers, RequestHead, RequestPayload, ResponseBody, ResponsePayload};
use async_trait::async_trait;
use http::StatusCode;

#[async_trait]
pub trait Router<RQ: RequestPayload + Send + Sync + ?Sized>: Sync {
    async fn route(&self, request_context: &mut RQ) -> Option<ResponsePayload>;
}

#[async_trait]
impl<Rtr: Sync + ?Sized + Router<RQ>, RQ: RequestPayload + Send + Sync> Router<RQ> for Box<Rtr> {
    async fn route(&self, request_context: &mut RQ) -> Option<ResponsePayload> {
        (**self).route(request_context).await
    }
}

pub struct PlainRequestPayload<'a> {
    request: &'a mut (dyn RequestPayload + Send + Sync),
}

impl<'a> PlainRequestPayload<'a> {
    pub fn new(request: &'a mut (dyn RequestPayload + Send + Sync)) -> Self {
        Self { request }
    }
}

impl<'a> RequestPayload for PlainRequestPayload<'a> {
    fn get_head(&self) -> &(dyn RequestHead + Send + Sync) {
        self.request.get_head()
    }

    fn take_body(&self) -> serde_json::Value {
        self.request.take_body()
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
    async fn route(&self, request_context: &mut RQ) -> Option<ResponsePayload> {
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
