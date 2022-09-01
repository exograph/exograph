use crate::{
    request_context::{ParsedContext, RequestContext},
    ResolveOperationFn,
};
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
        key: &str,
        _resolver: &ResolveOperationFn<'r>,
        _request_context: &'r RequestContext<'r>,
        request: &'r (dyn Request + Send + Sync),
    ) -> Option<Value> {
        request
            .get_header(&key.to_ascii_lowercase())
            .map(|str| str.as_str().into())
    }
}
