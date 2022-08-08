use async_trait::async_trait;
use serde_json::Value;

use crate::ResolveOperationFn;

use super::{ParsedContext, RequestContext};

pub struct EnvironmentContextExtractor;

#[async_trait]
impl ParsedContext for EnvironmentContextExtractor {
    fn annotation_name(&self) -> &str {
        "env"
    }

    async fn extract_context_field<'r>(
        &self,
        key: &str,
        _resolver: &ResolveOperationFn<'r>,
        _request_context: &'r RequestContext<'r>,
    ) -> Option<Value> {
        std::env::var(&key).ok().map(|v| v.into())
    }
}
