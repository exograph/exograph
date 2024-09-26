// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_trait::async_trait;
use serde_json::Value;

use crate::context::{context_extractor::ContextExtractor, ContextExtractionError, RequestContext};
use common::http::RequestHead;

pub struct IpExtractor;

#[async_trait]
impl ContextExtractor for IpExtractor {
    fn annotation_name(&self) -> &str {
        "clientIp"
    }

    async fn extract_context_field(
        &self,
        _key: &str,
        _request_context: &RequestContext,
        request_head: &(dyn RequestHead + Send + Sync),
    ) -> Result<Option<Value>, ContextExtractionError> {
        Ok(request_head.get_ip().map(|ip| ip.to_string().into()))
    }
}
