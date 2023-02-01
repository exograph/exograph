use async_trait::async_trait;

use crate::system_resolver::SystemResolver;
use crate::OperationsPayload;

use super::Request;
use super::{ParsedContext, RequestContext};

pub struct QueryExtractor<'a> {
    system_resolver: &'a SystemResolver,
}

impl<'a> QueryExtractor<'a> {
    pub fn new(system_resolver: &'a SystemResolver) -> QueryExtractor<'a> {
        QueryExtractor { system_resolver }
    }
}

#[async_trait]
impl ParsedContext for QueryExtractor<'_> {
    fn annotation_name(&self) -> &str {
        "query"
    }

    async fn extract_context_field<'r>(
        &self,
        key: Option<&str>,
        request_context: &'r RequestContext<'r>,
        _request: &'r (dyn Request + Send + Sync),
    ) -> Option<serde_json::Value> {
        let key = key?;
        let query = format!("query {{ {} }}", key.to_owned());

        let result = self
            .system_resolver
            .resolve_operations(
                OperationsPayload {
                    operation_name: None,
                    query,
                    variables: None,
                },
                request_context,
            )
            .await
            .ok()?;

        let (_, query_result) = result.iter().find(|(k, _)| k == key).unwrap_or_else(|| {
            panic!("Could not find {key} in results while processing @query context")
        });

        Some(
            query_result.body.to_json().expect(
                "Could not convert query result into JSON during @query context processing",
            ),
        )
    }
}
