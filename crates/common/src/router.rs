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
pub trait Router: Sync {
    async fn route(&self, request: &mut (dyn RequestPayload + Send)) -> Option<ResponsePayload>;
}

pub struct CompositeRouter {
    routers: Vec<Box<dyn Router + Send>>,
}

impl CompositeRouter {
    pub fn new(routers: Vec<Box<dyn Router + Send>>) -> Self {
        Self { routers }
    }
}

#[async_trait::async_trait]
impl Router for CompositeRouter {
    async fn route(&self, request: &mut (dyn RequestPayload + Send)) -> Option<ResponsePayload> {
        for router in self.routers.iter() {
            if let Some(response) = router.route(request).await {
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
