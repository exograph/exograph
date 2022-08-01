use actix_web::http::header::HeaderMap;
use async_trait::async_trait;
use payas_server_core::{
    request_context::{BoxedParsedContext, ParsedContext, RequestContext},
    ResolveFn,
};
use serde_json::Value;

use super::ActixContextProducer;

pub struct HeaderProcessor;

impl ActixContextProducer for HeaderProcessor {
    fn parse_context(
        &self,
        request: &actix_web::HttpRequest,
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
        key: &str,
        _resolver: &'s ResolveFn<'s, 'r>,
        _request_context: &'r RequestContext<'r>,
    ) -> Option<Value> {
        self.headers
            .get(&key.to_ascii_lowercase())
            .and_then(|v| v.to_str().ok())
            .map(|str| str.into())
    }
}
