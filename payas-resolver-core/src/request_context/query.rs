use async_trait::async_trait;

use crate::OperationsPayload;
use crate::ResolveOperationFn;

use super::Request;
use super::{ParsedContext, RequestContext};

pub struct QueryExtractor;

#[async_trait]
impl ParsedContext for QueryExtractor {
    fn annotation_name(&self) -> &str {
        "query"
    }

    async fn extract_context_field<'r>(
        &self,
        key: Option<&str>,
        resolver: &ResolveOperationFn<'r>,
        request_context: &'r RequestContext<'r>,
        _request: &'r (dyn Request + Send + Sync),
    ) -> Option<serde_json::Value> {
        let key = key?;
        let query = format!("query {{ {} }}", key.to_owned());

        let result = resolver.as_ref()(
            OperationsPayload {
                operation_name: None,
                query,
                variables: None,
            },
            request_context.into(),
        )
        .await
        .ok()?;

        let (_, query_result) = result.iter().find(|(k, _)| k == key).unwrap_or_else(|| {
            panic!(
                "Could not find {} in results while processing @query context",
                key
            )
        });

        Some(
            query_result.body.to_json().expect(
                "Could not convert query result into JSON during @query context processing",
            ),
        )
    }
}
