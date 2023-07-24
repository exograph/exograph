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

use super::{request::Request, ContextParsingError, RequestContext};

/// Extractor for a particular context field
///
/// This trait should be implemented on objects that represent a particular source of parsed context fields
#[async_trait]
pub trait ContextExtractor {
    // what annotation does this extractor provide values for?
    // e.g. "jwt", "header", etc.
    fn annotation_name(&self) -> &str;

    // extract a context field from this struct
    async fn extract_context_field<'r>(
        &self,
        key: &str,
        request_context: &RequestContext,
        request: &(dyn Request + Send + Sync),
    ) -> Result<Option<Value>, ContextParsingError>;
}
pub type BoxedParsedContext<'a> = Box<dyn ContextExtractor + 'a + Send + Sync>;

#[cfg(feature = "test-context")]
pub struct TestRequestContext {
    pub test_values: Value,
}

#[cfg(feature = "test-context")]
#[async_trait]
impl ContextExtractor for TestRequestContext {
    fn annotation_name(&self) -> &str {
        "test"
    }

    async fn extract_context_field<'r>(
        &self,
        key: &str,
        _request_context: &RequestContext,
        _request: &(dyn Request + Send + Sync),
    ) -> Result<Option<Value>, ContextParsingError> {
        Ok(self.test_values.get(key).cloned())
    }
}
