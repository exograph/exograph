mod environment;
mod query;

use std::collections::HashMap;

use async_trait::async_trait;
use futures::StreamExt;
use payas_model::model::ContextField;
use payas_model::model::ContextType;
use serde_json::Value;
use thiserror::Error;

use crate::ResolveOperationFn;

use self::{environment::EnvironmentContextExtractor, query::QueryExtractor};

use async_recursion::async_recursion;

#[derive(Debug, Error)]
pub enum ContextParsingError {
    #[error("Could not find source `{0}`")]
    SourceNotFound(String),

    #[error("{0}")]
    Delegate(#[from] Box<dyn std::error::Error + Send + Sync>),
}

pub type BoxedParsedContext = Box<dyn ParsedContext + Send + Sync>;

/// Represent a request context extracted for a particular request
pub struct UserRequestContext {
    // maps from an annotation to a parsed context
    parsed_context_map: HashMap<String, BoxedParsedContext>,
}

impl UserRequestContext {
    // Constructs a UserRequestContext from a vector of parsed contexts.
    pub fn from_parsed_contexts(contexts: Vec<BoxedParsedContext>) -> UserRequestContext {
        // a list of backend-agnostic contexts to also include

        let generic_contexts: Vec<BoxedParsedContext> = vec![
            Box::new(EnvironmentContextExtractor),
            Box::new(QueryExtractor),
        ];

        UserRequestContext {
            parsed_context_map: contexts
                .into_iter()
                .chain(generic_contexts.into_iter()) // include agnostic contexts
                .map(|context| (context.annotation_name().to_owned(), context))
                .collect(),
        }
    }
}

pub enum RequestContext<'a> {
    User(UserRequestContext),

    // The recursive nature allows stacking overrides
    Overridden {
        base_context: &'a RequestContext<'a>,
        context_override: Value,
    },
}

impl<'a> RequestContext<'a> {
    pub fn with_override(&'a self, context_override: Value) -> RequestContext<'a> {
        RequestContext::Overridden {
            base_context: self,
            context_override,
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
        resolver: &'s ResolveOperationFn<'a>,
        annotation_name: &str,
        value: &str,
    ) -> Result<Option<Value>, ContextParsingError> {
        let parsed_context = parsed_context_map
            .get(annotation_name)
            .ok_or_else(|| ContextParsingError::SourceNotFound(annotation_name.into()))?;

        Ok(parsed_context
            .extract_context_field(value, resolver, self)
            .await)
    }

    #[async_recursion]
    pub async fn extract_context_field<'s>(
        &'a self,
        context: &ContextType,
        field: &ContextField,
        resolver: &'s ResolveOperationFn<'a>,
    ) -> Result<Option<(String, Value)>, ContextParsingError> {
        match self {
            RequestContext::User(UserRequestContext { parsed_context_map }) => {
                let field_value = self
                    .extract_context_field_from_source(
                        parsed_context_map,
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
    ) -> Option<Value>;
}

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
    ) -> Option<Value> {
        self.test_values.get(key).cloned()
    }
}
