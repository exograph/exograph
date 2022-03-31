use payas_server_core::request_context::{BoxedParsedContext, ParsedContext};
use serde_json::Value;

use super::{ActixContextProducer, ContextProducerError};

pub struct EnvironmentProcessor;

impl ActixContextProducer for EnvironmentProcessor {
    fn parse_context(
        &self,
        _request: &actix_web::HttpRequest,
    ) -> Result<BoxedParsedContext, ContextProducerError> {
        Ok(Box::new(EnvironmentContextExtractor))
    }
}

struct EnvironmentContextExtractor;

impl ParsedContext for EnvironmentContextExtractor {
    fn annotation_name(&self) -> &str {
        "env"
    }

    fn extract_context_field(&self, key: &str) -> Option<Value> {
        std::env::var(&key).ok().map(|v| v.into())
    }
}
