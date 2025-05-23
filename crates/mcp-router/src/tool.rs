use async_trait::async_trait;
use http::StatusCode;

use common::context::RequestContext;
use core_resolver::plugin::subsystem_rpc_resolver::{JsonRpcRequest, SubsystemRpcError};

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> String;

    async fn description(
        &self,
        request_context: &RequestContext<'_>,
    ) -> Result<String, SubsystemRpcError>;

    fn input_schema(&self) -> serde_json::Value;

    async fn execute(
        &self,
        request: JsonRpcRequest,
        request_context: &RequestContext<'_>,
    ) -> Result<(Vec<String>, StatusCode, Vec<(String, String)>), SubsystemRpcError>;
}
