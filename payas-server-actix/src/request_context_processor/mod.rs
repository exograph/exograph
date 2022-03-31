pub mod environment;
pub mod header;
pub mod jwt;

use actix_web::HttpRequest;
use payas_server_core::request_context::{BoxedParsedContext, RequestContext};

use self::{environment::EnvironmentProcessor, header::HeaderProcessor, jwt::JwtAuthenticator};

pub trait ContextProcessor {
    fn parse_context(
        &self,
        request: &HttpRequest,
    ) -> Result<BoxedParsedContext, ContextProcessorError>;
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
        let parsed_contexts = self
            .processors
            .iter()
            .map(|processor| {
                // process values
                processor.parse_context(request)
            })
            .collect::<Result<Vec<_>, ContextProcessorError>>()?; // emit errors if we encounter any while gathering context

        Ok(RequestContext::from_parsed_contexts(parsed_contexts))
    }
}

impl Default for RequestContextProcessor {
    fn default() -> Self {
        Self::new()
    }
}
