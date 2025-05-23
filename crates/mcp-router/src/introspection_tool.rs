use std::sync::Arc;

use async_trait::async_trait;
use common::context::RequestContext;
use core_resolver::{
    plugin::subsystem_rpc_resolver::{JsonRpcRequest, SubsystemRpcError},
    system_resolver::GraphQLSystemResolver,
};
use http::StatusCode;
use serde_json::json;

use crate::{executor::Executor, tool::Tool};

pub struct IntrospectionTool {
    name: String,
    resolver: Arc<GraphQLSystemResolver>,
}

impl IntrospectionTool {
    pub fn new(name: String, resolver: Arc<GraphQLSystemResolver>) -> Self {
        Self { name, resolver }
    }
}

#[async_trait]
impl Tool for IntrospectionTool {
    fn name(&self) -> String {
        self.name.clone()
    }

    async fn description(
        &self,
        _request_context: &RequestContext<'_>,
    ) -> Result<String, SubsystemRpcError> {
        Ok(
            "Introspect the GraphQL schema to get supported queries and their arguments"
                .to_string(),
        )
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    async fn execute(
        &self,
        _request: JsonRpcRequest,
        request_context: &RequestContext<'_>,
    ) -> Result<(Vec<String>, StatusCode, Vec<(String, String)>), SubsystemRpcError> {
        let introspection_schema = self
            .resolver
            .get_introspection_schema(request_context)
            .await?;

        Ok((vec![introspection_schema], StatusCode::OK, vec![]))
    }
}
