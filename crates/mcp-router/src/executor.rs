use async_trait::async_trait;
use serde_json::{json, Map, Value};

use common::{context::RequestContext, operation_payload::OperationsPayload};
use core_plugin_shared::trusted_documents::TrustedDocumentEnforcement;
use core_resolver::{
    plugin::subsystem_rpc_resolver::SubsystemRpcError, system_resolver::GraphQLSystemResolver,
    QueryResponse,
};
use graphql_router::resolve_in_memory_for_payload;

#[async_trait]
pub trait Executor {
    async fn execute_query(
        &self,
        query: String,
        variables: Option<Map<String, Value>>,
        request_context: &RequestContext<'_>,
    ) -> Result<Vec<(String, QueryResponse)>, SubsystemRpcError>;

    async fn get_introspection_schema(
        &self,
        request_context: &RequestContext<'_>,
    ) -> Result<String, SubsystemRpcError>;
}

#[async_trait]
impl Executor for GraphQLSystemResolver {
    async fn execute_query(
        &self,
        query: String,
        variables: Option<Map<String, Value>>,
        request_context: &RequestContext<'_>,
    ) -> Result<Vec<(String, QueryResponse)>, SubsystemRpcError> {
        let operations_payload = OperationsPayload {
            query: Some(query),
            variables,
            operation_name: None,
            query_hash: None,
        };

        resolve_in_memory_for_payload(
            operations_payload,
            &self,
            TrustedDocumentEnforcement::DoNotEnforce,
            request_context,
        )
        .await
        .map_err(|e| {
            tracing::error!("Error while resolving request: {:?}", e);
            e.into()
        })
    }

    async fn get_introspection_schema(
        &self,
        request_context: &RequestContext<'_>,
    ) -> Result<String, SubsystemRpcError> {
        let query = introspection_util::get_introspection_query()
            .await
            .map_err(|_| SubsystemRpcError::InternalError)?;

        let query = query.as_str().ok_or(SubsystemRpcError::InternalError)?;

        let graphql_response = self
            .execute_query(query.to_string(), None, request_context)
            .await?;

        let (first_key, first_response) = graphql_response.first().unwrap();

        let schema_response = first_response
            .body
            .to_json()
            .map_err(|_| SubsystemRpcError::InternalError)?;

        let schema_response = json!({ "data": { first_key: schema_response } });

        introspection_util::schema_sdl(schema_response)
            .await
            .map_err(|_| SubsystemRpcError::InternalError)
    }
}
