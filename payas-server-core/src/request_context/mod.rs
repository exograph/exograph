use std::collections::HashMap;

use anyhow::anyhow;
use serde_json::Value;

pub type BoxedParsedContext = Box<dyn ParsedContext + Send + Sync>;

/// Represent a request context for a particular request
pub struct RequestContext {
    // maps from an annotation to a parsed context
    parsed_context_map: HashMap<String, BoxedParsedContext>,
}

impl RequestContext {
    // Constructs a RequestContext from a vector of parsed contexts.
    pub fn from_parsed_contexts(contexts: Vec<BoxedParsedContext>) -> RequestContext {
        RequestContext {
            parsed_context_map: contexts
                .into_iter()
                .map(|context| (context.annotation_name().to_owned(), context))
                .collect(),
        }
    }

    // Given an annotation name and its value,
    // extract a context field from the request context
    pub fn extract_context_field_from_source(
        &self,
        annotation_name: &str,
        value: &str,
    ) -> anyhow::Result<Option<Value>> {
        let parsed_context = self
            .parsed_context_map
            .get(annotation_name)
            .ok_or_else(|| anyhow!("Could not find source `{}`", annotation_name))?;

        Ok(parsed_context.extract_context_field(value))
    }
}

// Represents a parsed context
//
// Provides methods to extract context fields out of a given struct
// This trait should be implemented on objects that represent a particular source of parsed context fields
pub trait ParsedContext {
    // what annotation does this extractor provide values for?
    // e.g. "jwt", "header", etc.
    fn annotation_name(&self) -> &str;

    // extract a context field from this struct
    fn extract_context_field(&self, value: &str) -> Option<Value>;
}
