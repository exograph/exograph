use anyhow::Result;
use lambda_http::http::HeaderMap;
use payas_server_core::request_context::{BoxedParsedContext, ParsedContext};
use serde_json::Value;

use super::LambdaContextProducer;

pub struct HeaderProcessor;

impl LambdaContextProducer for HeaderProcessor {
    fn parse_context(
        &self,
        request: &lambda_http::Request,
    ) -> Result<BoxedParsedContext, super::ContextProducerError> {
        Ok(Box::new(ParsedHeaderContext {
            headers: request.headers().clone(),
        }))
    }
}

struct ParsedHeaderContext {
    headers: HeaderMap,
}

impl ParsedContext for ParsedHeaderContext {
    fn annotation_name(&self) -> &str {
        "header"
    }

    fn extract_context_field(&self, key: &str) -> Option<Value> {
        self.headers
            .get(&key.to_ascii_lowercase())
            .and_then(|v| v.to_str().ok())
            .map(|str| str.into())
    }
}
