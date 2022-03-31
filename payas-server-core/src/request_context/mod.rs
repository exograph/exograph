use std::collections::HashMap;

use anyhow::{anyhow, Result};
use payas_model::model::ContextType;
use serde_json::{Map, Value};

/// Represent a request context for a particular request
pub struct RequestContext {
    pub source_context_map: HashMap<String, Map<String, Value>>,
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
                        .get(&field.value.annotation)
                        .ok_or_else(|| {
                            anyhow!("No such annotation named {}", field.value.annotation)
                        })?
                        .get(&field.value.value)
                        .map(|v| (field.name.clone(), v.clone())))
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect(),
        ))
    }
}
