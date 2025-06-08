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

use crate::context::{ContextExtractionError, RequestContext, context_extractor::ContextExtractor};

pub struct EnvironmentContextExtractor;

#[async_trait]
impl ContextExtractor for EnvironmentContextExtractor {
    fn annotation_name(&self) -> &str {
        "env"
    }

    async fn extract_context_field(
        &self,
        key: &str,
        request_context: &RequestContext,
    ) -> Result<Option<Value>, ContextExtractionError> {
        Ok(request_context
            .system_context
            .env
            .get(key)
            .map(|v| v.as_str().into()))
    }
}
