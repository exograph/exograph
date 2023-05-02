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

use crate::context::{parsed_context::ParsedContext, request::Request, RequestContext};

pub struct IpExtractor;

#[async_trait]
impl ParsedContext for IpExtractor {
    fn annotation_name(&self) -> &str {
        "clientIp"
    }

    async fn extract_context_field<'r>(
        &self,
        _key: Option<&str>,
        _field_name: &str,
        _request_context: &'r RequestContext<'r>,
        request: &'r (dyn Request + Send + Sync),
    ) -> Option<Value> {
        request.get_ip().map(|ip| ip.to_string().into())
    }
}
