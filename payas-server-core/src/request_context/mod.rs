mod environment;
mod query;

use crate::OperationsExecutor;
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
pub struct RequestContext<'a> {
    #[cfg(not(test))]
    // maps from an annotation to a parsed context
    parsed_context_map: HashMap<String, BoxedParsedContext>,
    #[cfg(not(test))]
    executor: &'a OperationsExecutor,

    #[cfg(test)]
    test_values: serde_json::Value,
    #[cfg(test)]
    phantom: PhantomData<&'a ()>,
}

impl<'a> RequestContext<'a> {
    // Constructs a RequestContext from a vector of parsed contexts.
    #[cfg(not(test))]
    pub fn from_parsed_contexts(
        contexts: Vec<BoxedParsedContext>,
        executor: &'a OperationsExecutor,
    ) -> RequestContext<'a> {
        // a list of backend-agnostic contexts to also include

        let generic_contexts: Vec<BoxedParsedContext> = vec![
            Box::new(EnvironmentContextExtractor),
            Box::new(QueryExtractor),
        ];

        RequestContext {
            parsed_context_map: contexts
                .into_iter()
                .chain(generic_contexts.into_iter()) // include agnostic contexts
                .map(|context| (context.annotation_name().to_owned(), context))
                .collect(),
            executor,
        }
    }

    // Given an annotation name and its value,
    // extract a context field from the request context
    #[cfg(not(test))]
    async fn extract_context_field_from_source(
        &'a self,
        annotation_name: &str,
        value: &str,
    ) -> anyhow::Result<Option<Value>> {
        let parsed_context = self
            .parsed_context_map
            .get(annotation_name)
            .ok_or_else(|| anyhow!("Could not find source `{}`", annotation_name))?;

        Ok(parsed_context
            .extract_context_field(value, self.executor, self)
            .await)
    }

    #[cfg(not(test))]
    pub async fn extract_context(&self, context: &ContextType) -> Result<Value> {
        Ok(Value::Object(
            futures::stream::iter(context.fields.iter())
                .then(|field| async {
                    let field_value = self
                        .extract_context_field_from_source(
                            &field.source.annotation_name,
                            &field.source.value,
                        )
                        .await?;
                    Ok(field_value.map(|value| (field.name.clone(), value)))
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
        RequestContext {
            test_values,
            phantom: PhantomData,
        }
    }

    #[cfg(test)]
    pub async fn extract_context(&self, context: &ContextType) -> Result<Value> {
        self.test_values
            .get(&context.name)
            .ok_or(anyhow!(
                "Context type {} does not exist in test values",
                &context.name
            ))
            .cloned()
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
        executor: &'e OperationsExecutor,
        request_context: &'e RequestContext,
    ) -> Option<Value>;
}
