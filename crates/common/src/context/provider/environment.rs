// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::sync::Arc;

use async_trait::async_trait;
use exo_env::Environment;
use serde_json::Value;

use crate::context::{context_extractor::ContextExtractor, ContextExtractionError, RequestContext};

pub struct EnvironmentContextExtractor {
    pub env: Arc<dyn Environment>,
}

#[async_trait]
impl<'request> ContextExtractor<'request> for EnvironmentContextExtractor {
    fn annotation_name(&self) -> &str {
        "env"
    }

    async fn extract_context_field(
        &self,
        key: &str,
        _request_context: &'request RequestContext<'request>,
    ) -> Result<Option<Value>, ContextExtractionError> {
        Ok(self.env.get(key).map(|v| v.as_str().into()))
    }
}
