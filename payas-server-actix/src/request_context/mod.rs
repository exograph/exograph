pub mod cookie;
pub mod header;
pub mod jwt;

use actix_web::HttpRequest;
use payas_server_core::{
    request_context::{BoxedParsedContext, RequestContext, UserRequestContext},
    ResolveFn,
};

use self::{cookie::CookieProcessor, header::HeaderProcessor, jwt::JwtAuthenticator};

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
                Box::new(CookieProcessor),
                Box::new(JwtAuthenticator::new_from_env()),
                Box::new(HeaderProcessor),
            ],
        }
    }

    /// Generates request context
    pub fn generate_request_context<'s, 'a>(
        &self,
        request: &HttpRequest,
        resolve: &'s ResolveFn<'s, 'a>,
    ) -> Result<RequestContext<'a>, ContextProducerError> {
        let parsed_contexts = self
            .producers
            .iter()
            .map(|producer| {
                // create parsed context
                producer.parse_context(request)
            })
            .collect::<Result<Vec<_>, ContextProducerError>>()?; // emit errors if we encounter any while gathering context

        Ok(RequestContext::User(
            UserRequestContext::from_parsed_contexts(parsed_contexts),
        ))
    }
}

impl Default for ActixRequestContextProducer {
    fn default() -> Self {
        Self::new()
    }
}
