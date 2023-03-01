use async_trait::async_trait;
use serde_json::Value;

use super::{ParsedContext, Request, RequestContext};

pub struct EnvironmentContextExtractor;

#[async_trait]
impl ParsedContext for EnvironmentContextExtractor {
    fn annotation_name(&self) -> &str {
        "env"
    }

    async fn extract_context_field<'r>(
        &self,
        key: Option<&str>,
        field_name: &str,
        _request_context: &'r RequestContext<'r>,
        _request: &'r (dyn Request + Send + Sync),
    ) -> Option<Value> {
        std::env::var(key.unwrap_or(field_name))
            .ok()
            .map(|v| v.into())
    }
}
