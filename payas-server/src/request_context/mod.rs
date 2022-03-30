pub mod environment;
pub mod header;
pub mod jwt;

use std::collections::HashMap;

use actix_web::HttpRequest;
use anyhow::{anyhow, Result};
use payas_model::model::ContextType;
use serde_json::{Map, Value};

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

/// Represent a request context for a particular request
pub struct RequestContext {
    source_context_map: HashMap<String, Map<String, Value>>,
}

impl RequestContext {
    // Generate a more specific request context using the ContextType by picking fields from RequestContext
    pub fn generate_context_subset(&self, context: &ContextType) -> Result<Value> {
        Ok(Value::Object(
            context
                .fields
                .iter()
                .map(|field| {
                    Ok(self
                        .source_context_map
                        .get(&field.source.annotation)
                        .ok_or_else(|| {
                            anyhow!("No such annotation named {}", field.source.annotation)
                        })?
                        .get(&field.source.claim)
                        .map(|v| (field.name.clone(), v.clone())))
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect(),
        ))
    }
}
