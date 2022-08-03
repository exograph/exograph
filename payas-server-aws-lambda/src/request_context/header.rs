use async_trait::async_trait;
use lambda_http::http::HeaderMap;
use payas_resolver_core::{
    request_context::{BoxedParsedContext, ParsedContext, RequestContext},
    ResolveFn,
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

    async fn extract_context_field<'s, 'r>(
        &self,
        value: &str,
        _resolver: &'s ResolveFn<'r>,
        _request_context: &'r RequestContext<'r>,
    ) -> Option<Value> {
        self.headers
            .get(&value.to_ascii_lowercase())
            .and_then(|v| v.to_str().ok())
            .map(|str| str.into())
    }
}
