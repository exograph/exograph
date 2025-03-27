use std::sync::Arc;

use async_trait::async_trait;

use async_stream::try_stream;
use bytes::Bytes;

use common::{
    context::RequestContext,
    env_const::get_mcp_http_path,
    http::{Headers, RequestHead, ResponseBody, ResponsePayload},
    operation_payload::OperationsPayload,
    router::Router,
};
use core_plugin_shared::trusted_documents::TrustedDocumentEnforcement;
use core_resolver::{
    plugin::subsystem_rpc_resolver::{
        JsonRpcId, JsonRpcRequest, SubsystemRpcError, SubsystemRpcResponse,
    },
    system_resolver::GraphQLSystemResolver,
    QueryResponse, QueryResponseBody,
};

use exo_env::Environment;
use graphql_router::resolve_in_memory_for_payload;
use http::{Method, StatusCode};
use serde_json::json;

const ERROR_METHOD_NOT_FOUND_CODE: &str = "-32601";
const ERROR_METHOD_NOT_FOUND_MESSAGE: &str = "Method not found";

/// MCP router
///
/// Partially supports the new [Streamable HTTP](https://spec.modelcontextprotocol.io/specification/2025-03-26/basic/transports/#streamable-http)
/// protocol. Once this specification is finalized and the [official SDK](https://github.com/modelcontextprotocol/rust-sdk) supports it, we will
/// use types from that crate.
///
/// The implementation forwards requests to the GraphQL resolver.
pub struct McpRouter {
    api_path_prefix: String,
    system_resolver: Arc<GraphQLSystemResolver>,
}

impl McpRouter {
    pub fn new(env: Arc<dyn Environment>, system_resolver: Arc<GraphQLSystemResolver>) -> Self {
        Self {
            api_path_prefix: get_mcp_http_path(env.as_ref()).clone(),
            system_resolver,
        }
    }

    fn suitable(&self, request_head: &(dyn RequestHead + Sync)) -> bool {
        let method = request_head.get_method();

        request_head.get_path().starts_with(&self.api_path_prefix)
            && (method == Method::GET || method == Method::POST)
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
            .execute_query(query.to_string(), request_context)
            .await?;

        let (first_name, first_response) = graphql_response.first().unwrap();

        let schema_response = first_response
            .body
            .to_json()
            .map_err(|_| SubsystemRpcError::InternalError)?;

        let schema_response = json!({ "data": { first_name: schema_response } });

        introspection_util::schema_sdl(schema_response)
            .await
            .map_err(|_| SubsystemRpcError::InternalError)
    }

    async fn handle_initialize(
        &self,
        _request: JsonRpcRequest,
        _request_context: &RequestContext<'_>,
    ) -> Result<Option<SubsystemRpcResponse>, SubsystemRpcError> {
        let response = SubsystemRpcResponse {
            response: QueryResponse {
                body: QueryResponseBody::Json(json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {
                        }
                    },
                    "serverInfo": {
                        "name": "Exograph",
                        "version": "0.1.0",
                    }
                })),
                headers: vec![],
            },
            status_code: StatusCode::OK,
        };

        Ok(Some(response))
    }

    async fn handle_tools_list(
        &self,
        _request: JsonRpcRequest,
        _request_context: &RequestContext<'_>,
    ) -> Result<Option<SubsystemRpcResponse>, SubsystemRpcError> {
        let introspection_schema = self.get_introspection_schema(_request_context).await?;

        let response_body = json!({
            "tools": [{
                "name": "execute_graphql",
                "description": format!("Execute a GraphQL query per the following schema: {}", introspection_schema),
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The graphql query to execute"
                        },
                        "variables": {
                            "type": "object",
                            "description": "The variables to pass to the graphql query",
                            "properties": {},
                            "required": []
                        }
                    },
                    "required": ["query"]
                }
            }]
        });

        let response = SubsystemRpcResponse {
            response: QueryResponse {
                body: QueryResponseBody::Json(response_body),
                headers: vec![],
            },
            status_code: StatusCode::OK,
        };

        Ok(Some(response))
    }

    async fn execute_query(
        &self,
        query: String,
        request_context: &RequestContext<'_>,
    ) -> Result<Vec<(String, QueryResponse)>, SubsystemRpcError> {
        let operations_payload = OperationsPayload {
            query: Some(query),
            variables: None,
            operation_name: None,
            query_hash: None,
        };

        resolve_in_memory_for_payload(
            operations_payload,
            &self.system_resolver,
            TrustedDocumentEnforcement::DoNotEnforce,
            request_context,
        )
        .await
        .map_err(|e| {
            tracing::error!("Error while resolving request: {:?}", e);
            SubsystemRpcError::InternalError
        })
    }

    async fn handle_tools_call(
        &self,
        request: JsonRpcRequest,
        request_context: &RequestContext<'_>,
    ) -> Result<Option<SubsystemRpcResponse>, SubsystemRpcError> {
        let params = request.params;

        match params {
            Some(params) => {
                let arguments = params
                    .get("arguments")
                    .ok_or(SubsystemRpcError::InvalidRequest)?;
                let query = arguments
                    .get("query")
                    .ok_or(SubsystemRpcError::InvalidRequest)?
                    .as_str()
                    .ok_or(SubsystemRpcError::InvalidRequest)?;

                let graphql_response = self.execute_query(query.to_string(), request_context).await;

                let tool_result = match graphql_response {
                    Ok(graphql_response) => {
                        let response_contents = graphql_response
                            .into_iter()
                            .map(|(name, response)| {
                                let content_string = match response.body {
                                    QueryResponseBody::Json(value) => value.to_string(),
                                    QueryResponseBody::Raw(value) => value.unwrap_or_default(),
                                };

                                let text = json!({
                                    "name": name,
                                    "response": content_string,
                                })
                                .to_string();

                                json!({
                                    "text": text,
                                    "type": "text",
                                })
                            })
                            .collect::<Vec<_>>();

                        json!({
                            "content": response_contents,
                            "isError": false,
                        })
                    }
                    Err(e) => {
                        json!({
                            "content": [json!({
                                "text": format!("Error: {:?}", e),
                                "type": "text",
                            })],
                            "isError": true,
                        })
                    }
                };

                let response = SubsystemRpcResponse {
                    response: QueryResponse {
                        body: QueryResponseBody::Json(tool_result),
                        headers: vec![],
                    },
                    status_code: StatusCode::OK,
                };

                Ok(Some(response))
            }
            None => {
                return Err(SubsystemRpcError::InvalidRequest);
            }
        }
    }

    async fn handle_notifications(
        &self,
        _request: JsonRpcRequest,
        _request_context: &RequestContext<'_>,
    ) -> Result<Option<SubsystemRpcResponse>, SubsystemRpcError> {
        let response = SubsystemRpcResponse {
            response: QueryResponse {
                body: QueryResponseBody::Raw(None),
                headers: vec![],
            },
            status_code: StatusCode::OK,
        };
        Ok(Some(response))
    }

    async fn handle_prompts_list(
        &self,
        _request: JsonRpcRequest,
        _request_context: &RequestContext<'_>,
    ) -> Result<Option<SubsystemRpcResponse>, SubsystemRpcError> {
        let response = SubsystemRpcResponse {
            response: QueryResponse {
                body: QueryResponseBody::Json(json!({
                    "prompts": [],
                    "resources": []
                })),
                headers: vec![],
            },
            status_code: StatusCode::OK,
        };
        Ok(Some(response))
    }
}

#[async_trait]
impl<'a> Router<RequestContext<'a>> for McpRouter {
    async fn route(&self, request_context: &RequestContext<'a>) -> Option<ResponsePayload> {
        use common::http::RequestPayload;

        let head = request_context.get_head();

        if !self.suitable(head) {
            return None;
        }

        let body = request_context.take_body();

        let request: Result<JsonRpcRequest, _> =
            serde_json::from_value(body).map_err(|_| SubsystemRpcError::ParseError);

        let mut id = None;
        let mut headers = Headers::new();

        let response = {
            match request {
                Ok(request) => {
                    if request.jsonrpc != "2.0" {
                        Err(SubsystemRpcError::InvalidRequest)
                    } else {
                        id = request.id.clone();
                        let response: Result<Option<SubsystemRpcResponse>, SubsystemRpcError> =
                            match request.method.as_str() {
                                "initialize" => {
                                    self.handle_initialize(request, request_context).await
                                }
                                "notifications/initialized" | "notifications/cancelled" => {
                                    self.handle_notifications(request, request_context).await
                                }
                                "tools/list" => {
                                    self.handle_tools_list(request, request_context).await
                                }
                                "tools/call" => {
                                    self.handle_tools_call(request, request_context).await
                                }
                                "prompts/list" | "resources/list" => {
                                    self.handle_prompts_list(request, request_context).await
                                }
                                _ => Err(SubsystemRpcError::MethodNotFound(request.method)),
                            };

                        if let Ok(Some(response)) = &response {
                            for (key, value) in response.response.headers.iter() {
                                headers.insert(key.into(), value.into());
                            }
                        }

                        response
                    }
                }
                Err(_) => Err(SubsystemRpcError::ParseError),
            }
        };

        // TODO: Share this code with the rpc router

        let stream = try_stream! {
            macro_rules! emit_id_and_close {
                () => {
                    yield Bytes::from_static(br#", "id": "#);

                    match id {
                        Some(JsonRpcId::String(value)) => {
                            yield Bytes::from_static(br#"""#);
                            yield Bytes::from(value);
                            yield Bytes::from_static(br#"""#);
                        }
                        Some(JsonRpcId::Number(value)) => {
                            yield Bytes::from(value.to_string());
                        }
                        None => {
                            // If there is no id, we emit nothing (per JSON-RPC 2.0 notification spec)
                        }
                    };

                    yield Bytes::from_static(br#"}"#);
                };
            }

            match response {
                Ok(Some(response)) => {
                    // Emit the response only if it is not a notification (which has no id)
                    if let Some(_) = id {
                        yield Bytes::from_static(br#"{"jsonrpc": "2.0", "result": "#);

                        match response.response.body {
                            QueryResponseBody::Json(value) => yield Bytes::from(value.to_string()),
                            QueryResponseBody::Raw(Some(value)) => yield Bytes::from(value),
                            QueryResponseBody::Raw(None) => yield Bytes::from_static(b"null"),
                        };

                        emit_id_and_close!();
                    } else {
                        yield Bytes::new();
                    }
                },
                Ok(None) => {
                    yield Bytes::from_static(br#"{"error": {"code": "#);
                    yield Bytes::from_static(ERROR_METHOD_NOT_FOUND_CODE.as_bytes());
                    yield Bytes::from_static(br#", "message": ""#);
                    yield Bytes::from_static(ERROR_METHOD_NOT_FOUND_MESSAGE.as_bytes());
                    yield Bytes::from_static(br#""}"#);
                    emit_id_and_close!();
                },
                Err(err) => {
                    tracing::error!("Error while resolving request: {:?}", err);

                    yield Bytes::from_static(br#"{"error": {"code": "#);
                    yield Bytes::from_static(err.error_code_string().as_bytes());
                    yield Bytes::from_static(br#", "message": ""#);
                    yield Bytes::from(
                        err.user_error_message().unwrap_or_default()
                            .replace('\"', "")
                            .replace('\n', "; ")
                    );
                    yield Bytes::from_static(br#""}"#);
                    emit_id_and_close!();
                },
            }
        };

        Some(ResponsePayload {
            body: ResponseBody::Stream(Box::pin(stream)),
            headers,
            status_code: StatusCode::OK,
        })
    }
}
