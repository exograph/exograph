use std::collections::HashMap;

use anyhow::Result;
use payas_server_core::request_context::{BoxedParsedContext, ParsedContext};
use serde_json::Value;

use super::{ActixContextProducer, ContextProducerError};

pub struct HeaderProcessor;

impl ActixContextProducer for HeaderProcessor {
    fn parse_context(
        &self,
        request: &actix_web::HttpRequest,
    ) -> Result<BoxedParsedContext, super::ContextProducerError> {
        let headers = request
            .headers()
            .iter()
            .map(|(header_name, header_value)| {
                Ok((
                    header_name.to_string().to_ascii_lowercase(),
                    header_value
                        .to_str()
                        .map_err(|_| ContextProducerError::Malformed)?
                        .to_string(),
                ))
            })
            .collect::<Result<HashMap<_, _>, ContextProducerError>>()?;

        Ok(Box::new(ParsedHeaderContext { headers }))
    }
}

struct ParsedHeaderContext {
    headers: HashMap<String, String>,
}

impl ParsedContext for ParsedHeaderContext {
    fn annotation_name(&self) -> &str {
        "header"
    }

    fn extract_context_field(&self, key: &str) -> Option<Value> {
        self.headers
            .get(&key.to_ascii_lowercase())
            .map(|v| v.clone().into())
    }
}
