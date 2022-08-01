mod environment;
mod query;

use crate::graphql::execution_error::ExecutionError;
use async_trait::async_trait;
use futures::StreamExt;
use payas_model::model::ContextField;
use payas_model::model::ContextType;
use serde_json::Value;

use super::execution::system_context::ResolveFn;

#[cfg(test)]
use std::marker::PhantomData;

#[cfg(not(test))]
use self::{environment::EnvironmentContextExtractor, query::QueryExtractor};

#[cfg(not(test))]
use std::collections::HashMap;

use async_recursion::async_recursion;

pub type BoxedParsedContext = Box<dyn ParsedContext + Send + Sync>;

/// Represent a request context extracted for a particular request
///
/// UserRequestContext has two variants available: a regular version for normal use, and a test version
/// for payas-server-core unit tests. As we do not have a full OperationsExecutor to test functionality
/// like the access solver, a more basic RequestContext is used during `cargo test`. This test variant
/// may be constructed with RequestContext::test_request_context(value), `value` being a serde_json::Value
/// that represents the complete request context.
///
/// For example:
///
/// let context = UserRequestContext::test_request_context(
///     serde_json::json!({ "AccessContext": {"token1": "token_value", "token2": "token_value"} }),
/// );
pub struct UserRequestContext {
    #[cfg(not(test))]
    // maps from an annotation to a parsed context
    parsed_context_map: HashMap<String, BoxedParsedContext>,

    #[cfg(test)]
    test_values: serde_json::Value,
}

impl UserRequestContext {
    // Constructs a UserRequestContext from a vector of parsed contexts.
    #[cfg(not(test))]
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

    #[cfg(test)]
    pub fn test_request_context(test_values: serde_json::Value) -> UserRequestContext<'a> {
        UserRequestContext { test_values }
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
        resolver: &ResolveFn<'s, 'a>,
    ) -> Result<Value, ExecutionError> {
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
    #[cfg(not(test))]
    async fn extract_context_field_from_source<'s>(
        &'a self,
        parsed_context_map: &HashMap<String, BoxedParsedContext>,
        resolver: &'s ResolveFn<'s, 'a>,
        annotation_name: &str,
        value: &str,
    ) -> Result<Option<Value>, ExecutionError> {
        let parsed_context = parsed_context_map.get(annotation_name).ok_or_else(|| {
            ExecutionError::Generic(format!("Could not find source `{}`", annotation_name))
        })?;

        Ok(parsed_context
            .extract_context_field(value, resolver, self)
            .await)
    }

    #[cfg(not(test))]
    #[async_recursion]
    pub async fn extract_context_field<'s>(
        &'a self,
        context: &ContextType,
        field: &ContextField,
        resolver: &'s ResolveFn<'s, 'a>,
    ) -> Result<Option<(String, Value)>, ExecutionError> {
        match self {
            RequestContext::User(UserRequestContext { parsed_context_map }) => {
                let field_value = self
                    .extract_context_field_from_source(
                        parsed_context_map,
                        &resolver,
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

    // ### BELOW USED ONLY DURING UNIT TESTS ###

    #[cfg(test)]
    #[async_recursion]
    pub async fn extract_context_field(
        &self,
        context: &ContextType,
        field: &ContextField,
        resolver: &ResolveFn,
    ) -> Result<Option<(String, Value)>, ExecutionError> {
        match self {
            RequestContext::User(UserRequestContext { test_values, .. }) => {
                let context_value: Option<Value> = test_values.get(&context.name).cloned();

                Ok(context_value.and_then(|value| {
                    value.as_object().and_then(|context_value| {
                        let field_value = context_value.get(&field.name).cloned();
                        field_value.map(|field_value| (field.name.clone(), field_value))
                    })
                }))
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
    async fn extract_context_field<'s, 'r>(
        &self,
        value: &str,
        resolver: &'s ResolveFn<'s, 'r>,
        request_context: &'r RequestContext<'r>,
    ) -> Option<Value>;
}
