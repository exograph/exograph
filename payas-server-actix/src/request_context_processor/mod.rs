pub mod environment;
pub mod header;
pub mod jwt;

use std::collections::HashMap;

use actix_web::HttpRequest;
use payas_server_core::request_context::RequestContext;
use serde_json::{Value, Map};

use self::{environment::EnvironmentProcessor, header::HeaderProcessor, jwt::JwtAuthenticator};

pub trait ContextProcessor {
    fn annotation(&self) -> &str;
    fn process(&self, request: &HttpRequest)
        -> Result<Vec<(String, Value)>, ContextProcessorError>;
}
pub enum ContextProcessorError {
    Unauthorized,
    Malformed,
    Unknown,
}

pub struct RequestContextProcessor {
    processors: Vec<Box<dyn ContextProcessor + Send + Sync>>,
}

impl RequestContextProcessor {
    pub fn new() -> RequestContextProcessor {
        RequestContextProcessor {
            processors: vec![
                Box::new(JwtAuthenticator::new_from_env()),
                Box::new(HeaderProcessor),
                Box::new(EnvironmentProcessor),
            ],
        }
    }

    /// Generates request context
    pub fn generate_request_context(
        &self,
        request: &HttpRequest,
    ) -> Result<RequestContext, ContextProcessorError> {
        let source_context_map = self
            .processors
            .iter()
            .map(|processor| {
                // process the claims
                Ok((
                    processor.annotation().to_string(),
                    processor
                        .process(request)?
                        .into_iter()
                        .collect::<Map<_, _>>(),
                ))
            })
            .collect::<Result<Vec<_>, ContextProcessorError>>()? // emit errors if we encounter any while gathering context
            .into_iter()
            .collect::<HashMap<_, _>>();

        Ok(RequestContext { source_context_map })
    }
}

impl Default for RequestContextProcessor {
    fn default() -> Self {
        Self::new()
    }
}
