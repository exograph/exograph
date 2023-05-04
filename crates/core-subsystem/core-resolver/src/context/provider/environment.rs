// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::Value;

use crate::context::{parsed_context::ParsedContext, request::Request, RequestContext};

pub struct EnvironmentContextExtractor<'a> {
    pub env: &'a HashMap<String, String>,
}

#[async_trait]
impl<'a> ParsedContext for EnvironmentContextExtractor<'a> {
    fn annotation_name(&self) -> &str {
        "env"
    }

    async fn extract_context_field<'r>(
        &self,
        key: &str,
        _request_context: &'r RequestContext<'r>,
        _request: &'r (dyn Request + Send + Sync),
    ) -> Option<Value> {
        self.env.get(key).map(|v| v.as_str().into())
    }
}