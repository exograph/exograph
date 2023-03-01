use crate::request_context::{ParsedContext, RequestContext};
use async_trait::async_trait;
use serde_json::Value;

use super::Request;

pub struct HeaderExtractor;

#[async_trait]
impl ParsedContext for HeaderExtractor {
    fn annotation_name(&self) -> &str {
        "header"
    }

    async fn extract_context_field<'r>(
        &self,
        key: Option<&str>,
        field_name: &str,
        _request_context: &'r RequestContext<'r>,
        request: &'r (dyn Request + Send + Sync),
    ) -> Option<Value> {
        request
            .get_header(&key.unwrap_or(field_name).to_ascii_lowercase())
            .map(|str| str.as_str().into())
    }
}
