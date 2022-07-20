use actix_web::cookie::Cookie;
use async_trait::async_trait;
use payas_server_core::{
    request_context::{BoxedParsedContext, ParsedContext, RequestContext},
    SystemContext,
};
use serde_json::Value;

use super::{ActixContextProducer, ContextProducerError};

pub struct CookieProcessor;

impl ActixContextProducer for CookieProcessor {
    fn parse_context(
        &self,
        request: &actix_web::HttpRequest,
    ) -> Result<BoxedParsedContext, ContextProducerError> {
        Ok(Box::new(ParsedCookieContext {
            cookies: request
                .cookies()
                .map_err(|_| ContextProducerError::Malformed)?
                .clone(),
        }))
    }
}

struct ParsedCookieContext {
    cookies: Vec<Cookie<'static>>,
}

#[async_trait]
impl ParsedContext for ParsedCookieContext {
    fn annotation_name(&self) -> &str {
        "cookie"
    }

    async fn extract_context_field<'e>(
        &'e self,
        value: &str,
        _system_context: &'e SystemContext,
        _rc: &'e RequestContext,
    ) -> Option<Value> {
        self.cookies
            .iter()
            .find(|cookie| cookie.name() == value)
            .map(|cookie| cookie.value().to_string().into())
    }
}
