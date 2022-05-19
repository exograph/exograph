mod environment;
mod query;

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::StreamExt;
use payas_model::model::ContextType;
use serde_json::Value;

use crate::OperationsExecutor;

use self::{environment::EnvironmentContextExtractor, query::QueryExtractor};

pub type BoxedParsedContext = Box<dyn ParsedContext + Send + Sync>;

/// Represent a request context for a particular request
pub struct RequestContext<'a> {
    // maps from an annotation to a parsed context
    parsed_context_map: HashMap<String, BoxedParsedContext>,
    executor: &'a OperationsExecutor,
}

impl<'a> RequestContext<'a> {
    // Constructs a RequestContext from a vector of parsed contexts.
    pub fn from_parsed_contexts(
        contexts: Vec<BoxedParsedContext>,
        executor: &OperationsExecutor,
    ) -> RequestContext {
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
    pub async fn extract_context_field_from_source(
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
}

// Represents a parsed context
//
// Provides methods to extract context fields out of a given struct
// This trait should be implemented on objects that represent a particular source of parsed context fields
#[async_trait(?Send)]
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
