// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::http::{Headers, RequestPayload, ResponseBody, ResponsePayload};
use async_trait::async_trait;
use http::StatusCode;

#[async_trait]
pub trait Router<RQ: Send + Sync>: Sync {
    async fn route(
        &self,
        request: &(dyn RequestPayload + Send + Sync),
        request_context: &RQ,
    ) -> Option<ResponsePayload>;
}

#[async_trait]
impl<Rtr: Sync + ?Sized + Router<RQ>, RQ: Send + Sync> Router<RQ> for Box<Rtr> {
    async fn route(
        &self,
        request: &(dyn RequestPayload + Send + Sync),
        request_context: &RQ,
    ) -> Option<ResponsePayload> {
        (**self).route(request, request_context).await
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
impl<RQ: Send + Sync, Rtr: Router<RQ>> Router<RQ> for CompositeRouter<Rtr> {
    async fn route(
        &self,
        request: &(dyn RequestPayload + Send + Sync),
        request_context: &RQ,
    ) -> Option<ResponsePayload> {
        for router in self.routers.iter() {
            if let Some(response) = router.route(request, request_context).await {
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
