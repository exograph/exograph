use async_trait::async_trait;
use serde_json::Value;

use super::{request::Request, RequestContext};

// Represents a parsed context
//
// Provides methods to extract context fields out of a given struct
// This trait should be implemented on objects that represent a particular source of parsed context fields
#[async_trait]
pub trait ParsedContext {
    // what annotation does this extractor provide values for?
    // e.g. "jwt", "header", etc.
    fn annotation_name(&self) -> &str;

    // extract a context field from this struct
    async fn extract_context_field<'r>(
        &self,
        key: Option<&str>,
        field_name: &str,
        request_context: &'r RequestContext<'r>,
        request: &'r (dyn Request + Send + Sync),
    ) -> Option<Value>;
}
pub type BoxedParsedContext<'a> = Box<dyn ParsedContext + 'a + Send + Sync>;

#[cfg(feature = "test-context")]
pub struct TestRequestContext {
    pub test_values: Value,
}

#[cfg(feature = "test-context")]
#[async_trait]
impl ParsedContext for TestRequestContext {
    fn annotation_name(&self) -> &str {
        "test"
    }

    async fn extract_context_field<'r>(
        &self,
        key: Option<&str>,
        _field_name: &str,
        _request_context: &'r RequestContext<'r>,
        _request: &'r (dyn Request + Send + Sync),
    ) -> Option<Value> {
        self.test_values.get(key?).cloned()
    }
}
