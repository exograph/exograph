use payas_server_core::request_context::{BoxedParsedContext, ParsedContextExtractor};
use serde_json::Value;

use super::{ContextProcessor, ContextProcessorError};

pub struct EnvironmentProcessor;

impl ContextProcessor for EnvironmentProcessor {
    fn parse_context(
        &self,
        _request: &actix_web::HttpRequest,
    ) -> Result<BoxedParsedContext, ContextProcessorError> {
        Ok(Box::new(EnvironmentContextExtractor))
    }
}

struct EnvironmentContextExtractor;

impl ParsedContextExtractor for EnvironmentContextExtractor {
    fn annotation_name(&self) -> &str {
        "env"
    }

    fn extract_value(&self, key: &str) -> Option<Value> {
        std::env::var(&key).ok().map(|v| v.into())
    }
}
