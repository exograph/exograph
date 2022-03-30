use serde_json::Value;

use super::{ContextProcessor, ContextProcessorError};

pub struct EnvironmentProcessor;

impl ContextProcessor for EnvironmentProcessor {
    fn annotation(&self) -> &str {
        "env"
    }

    fn process(
        &self,
        _request: &actix_web::HttpRequest,
    ) -> Result<Vec<(String, Value)>, ContextProcessorError> {
        Ok(std::env::vars()
            .map(|(name, var)| (name, var.into()))
            .collect())
    }
}
