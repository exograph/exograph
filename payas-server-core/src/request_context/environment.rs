use async_trait::async_trait;
use serde_json::Value;

use crate::execution::operations_context::OperationsContext;

use super::{ParsedContext, RequestContext};

pub struct EnvironmentContextExtractor;

#[async_trait]
impl ParsedContext for EnvironmentContextExtractor {
    fn annotation_name(&self) -> &str {
        "env"
    }

    async fn extract_context_field<'e>(
        &'e self,
        key: &str,
        _operations_context: &'e OperationsContext,
        _rc: &'e RequestContext,
    ) -> Option<Value> {
        std::env::var(&key).ok().map(|v| v.into())
    }
}
