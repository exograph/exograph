mod cookie;
mod environment;
mod header;
mod jwt;
mod query;

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use async_trait::async_trait;
use futures::StreamExt;
use payas_model::model::ContextField;
use payas_model::model::ContextType;
use payas_sql::TransactionHolder;
use serde_json::Value;
use thiserror::Error;

use crate::ResolveOperationFn;

use self::cookie::CookieExtractor;
use self::header::HeaderExtractor;
use self::jwt::JwtAuthenticator;
use self::{environment::EnvironmentContextExtractor, query::QueryExtractor};

use async_recursion::async_recursion;

#[derive(Debug, Error)]
pub enum ContextParsingError {
    #[error("Could not find source `{0}`")]
    SourceNotFound(String),

    #[error("Unauthorized request")]
    Unauthorized,

    #[error("Malformed request")]
    Malformed,

    #[error("{0}")]
    Delegate(#[from] Box<dyn std::error::Error + Send + Sync>),
}

/// Represents a HTTP request from which information can be extracted
pub trait Request {
    // return all header values that have the following key
    fn get_headers(&self, key: &str) -> Vec<String>;

    // return the first header
    fn get_header(&self, key: &str) -> Option<String> {
        self.get_headers(&key.to_lowercase()).get(0).cloned()
    }

    // return the IP address used to make the request
    fn get_ip(&self) -> Option<std::net::IpAddr>;
}

/// Represent a request context extracted for a particular request
pub struct UserRequestContext<'a> {
    // maps from an annotation to a parsed context
    parsed_context_map: HashMap<String, BoxedParsedContext>,
    pub transaction_holder: Arc<Mutex<TransactionHolder>>,
    request: &'a (dyn Request + Send + Sync),
}

impl<'a> UserRequestContext<'a> {
    // Constructs a UserRequestContext from a vector of parsed contexts and a request.
    pub fn parse_context(
        request: &'a (dyn Request + Send + Sync),
        parsed_contexts: Vec<BoxedParsedContext>,
    ) -> Result<UserRequestContext<'a>, ContextParsingError> {
        // a list of backend-agnostic contexts to also include
        let generic_contexts: Vec<BoxedParsedContext> = vec![
            Box::new(EnvironmentContextExtractor),
            Box::new(QueryExtractor),
            Box::new(HeaderExtractor),
            CookieExtractor::parse_context(request)?,
            JwtAuthenticator::parse_context(request)?,
        ];

        Ok(UserRequestContext {
            parsed_context_map: parsed_contexts
                .into_iter()
                .chain(
                    generic_contexts.into_iter(), // include agnostic contexts
                )
                .map(|context| (context.annotation_name().to_owned(), context))
                .collect(),
            transaction_holder: Arc::new(Mutex::new(TransactionHolder::default())),
            request,
        })
    }
}

pub enum RequestContext<'a> {
    User(UserRequestContext<'a>),

    // The recursive nature allows stacking overrides
    Overridden {
        base_context: &'a RequestContext<'a>,
        context_override: Value,
    },
}

impl<'a> RequestContext<'a> {
    pub fn parse_context(
        request: &'a (dyn Request + Send + Sync),
        parsed_contexts: Vec<BoxedParsedContext>,
    ) -> Result<RequestContext<'a>, ContextParsingError> {
        Ok(RequestContext::User(UserRequestContext::parse_context(
            request,
            parsed_contexts,
        )?))
    }

    pub fn with_override(&'a self, context_override: Value) -> RequestContext<'a> {
        RequestContext::Overridden {
            base_context: self,
            context_override,
        }
    }

    pub fn get_base_context(&self) -> &UserRequestContext {
        match &self {
            RequestContext::User(req) => req,

            RequestContext::Overridden { base_context, .. } => base_context.get_base_context(),
        }
    }

    pub async fn extract_context<'s>(
        &'a self,
        context: &ContextType,
        resolver: &ResolveOperationFn<'a>,
    ) -> Result<Value, ContextParsingError> {
        Ok(Value::Object(
            futures::stream::iter(context.fields.iter())
                .then(|field| async { self.extract_context_field(context, field, resolver).await })
                .collect::<Vec<Result<_, _>>>()
                .await
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect(),
        ))
    }

    // Given an annotation name and its value,
    // extract a context field from the request context
    async fn extract_context_field_from_source<'s>(
        &'a self,
        parsed_context_map: &HashMap<String, BoxedParsedContext>,
        request: &'a (dyn Request + Send + Sync),
        resolver: &'s ResolveOperationFn<'a>,
        annotation_name: &str,
        value: &str,
    ) -> Result<Option<Value>, ContextParsingError> {
        let parsed_context = parsed_context_map
            .get(annotation_name)
            .ok_or_else(|| ContextParsingError::SourceNotFound(annotation_name.into()))?;

        Ok(parsed_context
            .extract_context_field(value, resolver, self, request)
            .await)
    }

    #[async_recursion]
    async fn extract_context_field<'s>(
        &'a self,
        context: &ContextType,
        field: &ContextField,
        resolver: &'s ResolveOperationFn<'a>,
    ) -> Result<Option<(String, Value)>, ContextParsingError> {
        match self {
            RequestContext::User(UserRequestContext {
                parsed_context_map,
                request,
                ..
            }) => {
                let field_value = self
                    .extract_context_field_from_source(
                        parsed_context_map,
                        *request,
                        resolver,
                        &field.source.annotation_name,
                        &field.source.value,
                    )
                    .await?;
                Ok(field_value.map(|value| (field.name.clone(), value)))
            }
            RequestContext::Overridden {
                base_context,
                context_override,
            } => {
                let overridden: Option<&Value> = context_override
                    .get(&context.name)
                    .and_then(|value| value.as_object().and_then(|value| value.get(&field.name)));

                match overridden {
                    Some(value) => Ok(Some((field.name.clone(), value.clone()))),
                    None => {
                        base_context
                            .extract_context_field(context, field, resolver)
                            .await
                    }
                }
            }
        }
    }
}

// Represents a parsed context
//
// Provides methods to extract context fields out of a given struct
// This trait should be implemented on objects that represent a particular source of parsed context fields
#[async_trait]
pub trait ParsedContext {
    // what annotation does this extractor provide values for?
    // e.g. "jwt", "header", etc.
    fn annotation_name(&self) -> &str;

    // extract a context field from this struct
    async fn extract_context_field<'r>(
        &self,
        value: &str,
        resolver: &ResolveOperationFn<'r>,
        request_context: &'r RequestContext<'r>,
        request: &'r (dyn Request + Send + Sync),
    ) -> Option<Value>;
}
pub type BoxedParsedContext = Box<dyn ParsedContext + Send + Sync>;

#[cfg(test)]
pub struct TestRequestContext {
    pub test_values: Value,
}

#[cfg(test)]
#[async_trait]
impl ParsedContext for TestRequestContext {
    fn annotation_name(&self) -> &str {
        "test"
    }

    async fn extract_context_field<'r>(
        &self,
        key: &str,
        _resolver: &ResolveOperationFn<'r>,
        _request_context: &'r RequestContext<'r>,
        _request: &'r (dyn Request + Send + Sync),
    ) -> Option<Value> {
        self.test_values.get(key).cloned()
    }
}
