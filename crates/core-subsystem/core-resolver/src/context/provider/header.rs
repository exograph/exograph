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

use crate::context::{
    parsed_context::ParsedContext, request::Request, ContextParsingError, RequestContext,
};

pub struct HeaderExtractor;

#[async_trait]
impl ParsedContext for HeaderExtractor {
    fn annotation_name(&self) -> &str {
        "header"
    }

    async fn extract_context_field<'r>(
        &self,
        key: &str,
        _request_context: &'r RequestContext<'r>,
        request: &(dyn Request + Send + Sync),
    ) -> Result<Option<Value>, ContextParsingError> {
        Ok(request
            .get_header(&key.to_ascii_lowercase())
            .map(|str| str.as_str().into()))
    }
}
