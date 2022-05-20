use anyhow::Result;
use async_trait::async_trait;
use lambda_http::http::HeaderMap;
use payas_server_core::{
    request_context::{BoxedParsedContext, ParsedContext, RequestContext},
    OperationsExecutor,
};
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

#[async_trait]
impl ParsedContext for ParsedHeaderContext {
    fn annotation_name(&self) -> &str {
        "header"
    }

    async fn extract_context_field<'e>(
        &'e self,
        value: &str,
        _executor: &'e OperationsExecutor,
        _request_context: &'e RequestContext<'e>,
    ) -> Option<Value> {
        self.headers
            .get(&value.to_ascii_lowercase())
            .and_then(|v| v.to_str().ok())
            .map(|str| str.into())
    }
}
