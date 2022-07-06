use anyhow::Result;
use async_trait::async_trait;
use cookie::Cookie;
use payas_server_core::{
    request_context::{BoxedParsedContext, ParsedContext, RequestContext},
    OperationsContext,
};
use serde_json::Value;

use super::{ContextProducerError, LambdaContextProducer};

pub struct CookieProcessor;

impl LambdaContextProducer for CookieProcessor {
    fn parse_context(
        &self,
        request: &lambda_http::Request,
    ) -> Result<BoxedParsedContext, ContextProducerError> {
        let cookies: Vec<Cookie<'static>> = request
            .headers()
            .get_all("Cookie")
            .iter()
            .map(|cookie_header| {
                let cookie_strings = cookie_header
                    .to_str()
                    .map_err(|_| ContextProducerError::Malformed)?
                    .split(';');

                Ok(cookie_strings
                    .map(Cookie::parse)
                    .collect::<Result<Vec<Cookie<'_>>, cookie::ParseError>>()
                    .map_err(|_| ContextProducerError::Malformed)?
                    .into_iter()
                    .map(Cookie::into_owned)
                    .collect::<Vec<Cookie<'static>>>())
            })
            .collect::<Result<Vec<_>, ContextProducerError>>()?
            .into_iter()
            .flatten()
            .collect();

        Ok(Box::new(ParsedCookieContext { cookies }))
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
        _operations_context: &'e OperationsContext,
        _request_context: &'e RequestContext<'e>,
    ) -> Option<Value> {
        self.cookies
            .iter()
            .find(|cookie| cookie.name() == value)
            .map(|cookie| cookie.value().to_string().into())
    }
}
