use payas_server_core::request_context::{BoxedParsedContext, ParsedContext};
use serde_json::Value;

use super::{ContextProducerError, LambdaContextProducer};

pub struct EnvironmentProcessor;

impl LambdaContextProducer for EnvironmentProcessor {
    fn parse_context(
        &self,
        _request: &lambda_http::Request,
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
