pub mod environment;
pub mod header;
pub mod jwt;

use actix_web::HttpRequest;
use payas_server_core::request_context::{BoxedParsedContext, RequestContext};

use self::{environment::EnvironmentProcessor, header::HeaderProcessor, jwt::JwtAuthenticator};

pub trait ActixContextProducer {
    fn parse_context(
        &self,
        request: &HttpRequest,
    ) -> Result<BoxedParsedContext, ContextProducerError>;
}
pub enum ContextProducerError {
    Unauthorized,
    Malformed,
    Unknown,
}

pub struct ActixRequestContextProducer {
    producers: Vec<Box<dyn ActixContextProducer + Send + Sync>>,
}

impl ActixRequestContextProducer {
    pub fn new() -> ActixRequestContextProducer {
        ActixRequestContextProducer {
            producers: vec![
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
    ) -> Result<RequestContext, ContextProducerError> {
        let parsed_contexts = self
            .producers
            .iter()
            .map(|producer| {
                // create parsed context
                producer.parse_context(request)
            })
            .collect::<Result<Vec<_>, ContextProducerError>>()?; // emit errors if we encounter any while gathering context

        Ok(RequestContext::from_parsed_contexts(parsed_contexts))
    }
}

impl Default for ActixRequestContextProducer {
    fn default() -> Self {
        Self::new()
    }
}
