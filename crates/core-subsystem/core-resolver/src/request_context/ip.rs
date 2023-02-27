use crate::request_context::{ParsedContext, RequestContext};
use async_trait::async_trait;
use serde_json::Value;

use super::Request;

pub struct IpExtractor;

#[async_trait]
impl ParsedContext for IpExtractor {
    fn annotation_name(&self) -> &str {
        "clientIp"
    }

    async fn extract_context_field<'r>(
        &self,
        _key: Option<&str>,
        _field_name: &str,
        _request_context: &'r RequestContext<'r>,
        request: &'r (dyn Request + Send + Sync),
    ) -> Option<Value> {
        request.get_ip().map(|ip| ip.to_string().into())
    }
}
