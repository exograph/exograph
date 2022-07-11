mod environment;
mod query;

use crate::execution::system_context::SystemContext;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use payas_model::model::ContextType;
use serde_json::Value;

#[cfg(test)]
use std::marker::PhantomData;

#[cfg(not(test))]
use self::{environment::EnvironmentContextExtractor, query::QueryExtractor};
#[cfg(not(test))]
use futures::StreamExt;
#[cfg(not(test))]
use std::collections::HashMap;

use async_recursion::async_recursion;

pub type BoxedParsedContext = Box<dyn ParsedContext + Send + Sync>;

/// Represent a request context for a particular request
///
/// RequestContext has two variants available: a regular version for normal use, and a test version
/// for payas-server-core unit tests. As we do not have a full OperationsExecutor to test functionality
/// like the access solver, a more basic RequestContext is used during `cargo test`. This test variant
/// may be constructed with RequestContext::test_request_context(value), `value` being a serde_json::Value
/// that represents the complete request context.
///
/// For example:
///
/// let context = RequestContext::test_request_context(
///     serde_json::json!({ "AccessContext": {"token1": "token_value", "token2": "token_value"} }),
/// );

pub struct UserRequestContext<'a> {
    #[cfg(not(test))]
    // maps from an annotation to a parsed context
    parsed_context_map: HashMap<String, BoxedParsedContext>,
    #[cfg(not(test))]
    system_context: &'a SystemContext,

    #[cfg(test)]
    test_values: serde_json::Value,
    #[cfg(test)]
    phantom: PhantomData<&'a ()>,
}

pub enum RequestContext<'a> {
    User(UserRequestContext<'a>),

    Service {
        user_context: &'a UserRequestContext<'a>,
        context_override: Value,
    },
}

impl<'a> RequestContext<'a> {
    // Constructs a RequestContext from a vector of parsed contexts.
    #[cfg(not(test))]
    pub fn from_parsed_contexts(
        contexts: Vec<BoxedParsedContext>,
        system_context: &'a SystemContext,
    ) -> RequestContext<'a> {
        // a list of backend-agnostic contexts to also include

        let generic_contexts: Vec<BoxedParsedContext> = vec![
            Box::new(EnvironmentContextExtractor),
            Box::new(QueryExtractor),
        ];

        RequestContext::User(UserRequestContext {
            parsed_context_map: contexts
                .into_iter()
                .chain(generic_contexts.into_iter()) // include agnostic contexts
                .map(|context| (context.annotation_name().to_owned(), context))
                .collect(),
            system_context,
        })
    }

    pub fn with_override(&'a self, context_override: Value) -> RequestContext<'a> {
        match self {
            RequestContext::User(user_context) => RequestContext::Service {
                user_context,
                context_override,
            },
            RequestContext::Service { .. } => todo!(), // We could merge the two contexts
        }
    }

    // Given an annotation name and its value,
    // extract a context field from the request context
    #[cfg(not(test))]
    #[async_recursion]
    async fn extract_context_field_from_source(
        &self,
        parsed_context_map: &HashMap<String, BoxedParsedContext>,
        system_context: &'a SystemContext,
        annotation_name: &str,
        value: &str,
    ) -> Result<Option<Value>> {
        let parsed_context = parsed_context_map
            .get(annotation_name)
            .ok_or_else(|| anyhow!("Could not find source `{}`", annotation_name))?;

        Ok(parsed_context
            .extract_context_field(value, system_context, self)
            .await)
    }

    #[cfg(not(test))]
    pub async fn extract_context(&self, context: &ContextType) -> Result<Value> {
        Ok(Value::Object(
            futures::stream::iter(context.fields.iter())
                .then(|field| async {
                    match self {
                        RequestContext::User(UserRequestContext {
                            parsed_context_map,
                            system_context,
                        }) => {
                            let field_value = self
                                .extract_context_field_from_source(
                                    parsed_context_map,
                                    system_context,
                                    &field.source.annotation_name,
                                    &field.source.value,
                                )
                                .await?;
                            Ok(field_value.map(|value| (field.name.clone(), value)))
                        }
                        RequestContext::Service {
                            user_context,
                            context_override,
                        } => {
                            let overridden: Option<&Value> =
                                context_override.get(&context.name).and_then(|value| {
                                    value.as_object().and_then(|value| value.get(&field.name))
                                });

                            match overridden {
                                Some(value) => Ok(Some((field.name.clone(), value.clone()))),
                                None => {
                                    let field_value = self
                                        .extract_context_field_from_source(
                                            &user_context.parsed_context_map,
                                            user_context.system_context,
                                            &field.source.annotation_name,
                                            &field.source.value,
                                        )
                                        .await?;
                                    Ok(field_value.map(|value| (field.name.clone(), value)))
                                }
                            }
                        }
                    }
                })
                .collect::<Vec<Result<_>>>()
                .await
                .into_iter()
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect(),
        ))
    }

    // ### BELOW USED ONLY DURING UNIT TESTS ###

    #[cfg(test)]
    pub fn test_request_context(test_values: serde_json::Value) -> RequestContext<'a> {
        RequestContext::User(UserRequestContext {
            test_values,
            phantom: PhantomData,
        })
    }

    #[cfg(test)]
    #[async_recursion]
    pub async fn extract_context(&self, context: &ContextType) -> Result<Value> {
        match self {
            RequestContext::User(UserRequestContext {
                test_values,
                phantom: _,
            }) => test_values
                .get(&context.name)
                .ok_or_else(|| {
                    anyhow!(
                        "Context type {} does not exist in test values",
                        &context.name
                    )
                })
                .cloned(),
            RequestContext::Service {
                user_context,
                context_override,
            } => {
                let overridden = context_override
                    .as_object()
                    .and_then(|context_override_map| context_override_map.get(&context.name))
                    .cloned();
                match overridden {
                    Some(overridden) => Ok(overridden),
                    None => user_context
                        .test_values
                        .get(&context.name)
                        .ok_or_else(|| {
                            anyhow!(
                                "Context type {} does not exist in test values",
                                &context.name
                            )
                        })
                        .cloned(),
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
    async fn extract_context_field<'e>(
        &'e self,
        value: &str,
        system_context: &'e SystemContext,
        request_context: &'e RequestContext,
    ) -> Option<Value>;
}
