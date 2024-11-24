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

pub struct HeaderExtractor;

#[async_trait]
impl<'request> ContextExtractor<'request> for HeaderExtractor {
    fn annotation_name(&self) -> &str {
        "header"
    }

    async fn extract_context_field(
        &self,
        key: &str,
        request_context: &'request RequestContext<'request>,
    ) -> Result<Option<Value>, ContextExtractionError> {
        Ok(request_context
            .get_base_context()
            .get_head()
            .get_header(&key.to_ascii_lowercase())
            .map(|str| str.as_str().into()))
    }
}
