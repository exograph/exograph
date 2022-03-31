use std::collections::HashMap;

use anyhow::anyhow;
use serde_json::Value;

pub type BoxedParsedContext = Box<dyn ParsedContextExtractor + Send + Sync>;

/// Represent a request context for a particular request
pub struct RequestContext {
    parsed_context_map: HashMap<String, BoxedParsedContext>,
}

impl RequestContext {
    pub fn from_parsed_contexts(contexts: Vec<BoxedParsedContext>) -> RequestContext {
        RequestContext {
            parsed_context_map: contexts
                .into_iter()
                .map(|context| (context.annotation_name().to_owned(), context))
                .collect(),
        }
    }

    pub fn extract_value_from_source(
        &self,
        annotation_name: &str,
        key: &str,
    ) -> anyhow::Result<Option<Value>> {
        let parsed_context = self
            .parsed_context_map
            .get(annotation_name)
            .ok_or(anyhow!("Could not find source `{}`", annotation_name))?;

        Ok(parsed_context.extract_value(key))
    }
}

pub trait ParsedContextExtractor {
    fn annotation_name(&self) -> &str;
    fn extract_value(&self, key: &str) -> Option<Value>;
}
