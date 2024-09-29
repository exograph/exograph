// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::http::{RequestHead, RequestPayload, ResponsePayload};
use async_trait::async_trait;

#[async_trait]
pub trait ApiRouter: Sync {
    async fn suitable(&self, request_head: &(dyn RequestHead + Sync)) -> bool;

    async fn route(
        &self,
        request: impl RequestPayload + Send,
        playground_request: bool,
    ) -> ResponsePayload;
}
