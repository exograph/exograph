use std::sync::Arc;

use async_trait::async_trait;

use async_stream::try_stream;
use bytes::Bytes;

use crate::{
    error::McpRouterError, protocol_version::ProtocolVersion, tool::Tool,
    tools_creator::create_tools,
};

use common::{
    context::RequestContext,
    env_const::get_mcp_http_path,
    http::{Headers, RequestHead, ResponseBody, ResponsePayload},
    router::Router,
};
use core_plugin_shared::profile::{SchemaProfile, SchemaProfiles};
use core_resolver::{
    QueryResponse, QueryResponseBody,
    plugin::subsystem_rpc_resolver::{
        JsonRpcId, JsonRpcRequest, SubsystemRpcError, SubsystemRpcResponse,
    },
    system_resolver::GraphQLSystemResolver,
};

use core_router::SystemLoadingError;
use exo_env::Environment;
use http::{Method, StatusCode};
use serde_json::json;

const ERROR_METHOD_NOT_FOUND_CODE: &str = "-32601";
const ERROR_METHOD_NOT_FOUND_MESSAGE: &str = "Method not found";

/// MCP router
///
/// Partially supports the new [Streamable HTTP](https://spec.modelcontextprotocol.io/specification/2025-03-26/basic/transports/#streamable-http)
/// protocol.
///
/// The implementation forwards requests to the GraphQL resolver.
pub struct McpRouter {
    api_path_prefix: String,
    tools: Vec<Box<dyn Tool>>,
}

impl McpRouter {
    pub fn new(
        env: Arc<dyn Environment>,
        create_resolver: impl Fn(
            &SchemaProfile,
        ) -> Result<Arc<GraphQLSystemResolver>, SystemLoadingError>,
        schema_profiles: Option<SchemaProfiles>,
    ) -> Result<Self, SystemLoadingError> {
        Ok(Self {
            api_path_prefix: get_mcp_http_path(env.as_ref()).clone(),
            tools: create_tools(env.as_ref(), schema_profiles, &create_resolver)?,
        })
    }

    fn suitable(&self, request_head: &(dyn RequestHead + Sync)) -> bool {
        let method = request_head.get_method();

        request_head.get_path().starts_with(&self.api_path_prefix)
            && (method == Method::GET || method == Method::POST)
    }

    async fn handle_initialize(
        &self,
        request: JsonRpcRequest,
        _request_context: &RequestContext<'_>,
    ) -> Result<Option<SubsystemRpcResponse>, SubsystemRpcError> {
        let client_protocol_version: Result<ProtocolVersion, McpRouterError> = request
            .params
            .as_ref()
            .and_then(|params| {
                params
                    .get("protocolVersion")
                    .and_then(|v| v.as_str())
                    .map(|s| ProtocolVersion::try_from(s))
            })
            .unwrap_or(Ok(ProtocolVersion::V2024_11_05));

        // The [spec](https://modelcontextprotocol.io/specification/2025-03-26/basic/lifecycle#version-negotiation)
        // requires that if we support the client's version, we return that to be the server's version.
        //
        // We support every version listed in `ProtocolVersion`, so return the client's version
        // if it is valid.
        let server_protocol_version = match client_protocol_version {
            Ok(version) => version,
            Err(e) => {
                tracing::error!("Error parsing client protocol version: {:?}", e);
                return Err(SubsystemRpcError::InvalidRequest);
            }
        };

        let response = SubsystemRpcResponse {
            response: QueryResponse {
                body: QueryResponseBody::Json(json!({
                    "protocolVersion": server_protocol_version.to_string(),
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
        let mut tools_payload = Vec::new();

        for tool in &self.tools {
            tools_payload.push(json!({
                "name": tool.name(),
                "description": tool.description(_request_context).await?,
                "inputSchema": tool.input_schema(),
            }));
        }

        let response_body = json!({
            "tools": tools_payload,
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

    async fn handle_tools_call(
        &self,
        request: JsonRpcRequest,
        request_context: &RequestContext<'_>,
    ) -> Result<Option<SubsystemRpcResponse>, SubsystemRpcError> {
        let params = &request.params;

        match params {
            Some(params) => {
                let name = params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();

                let tool = self.tools.iter().find(|tool| tool.name() == name);

                match tool {
                    Some(tool) => {
                        let (tool_result, status_code, extra_headers) =
                            tool.execute(request, request_context).await?;

                        let content = tool_result
                            .into_iter()
                            .map(|result| {
                                json!({
                                    "text": result,
                                    "type": "text",
                                })
                            })
                            .collect::<Vec<_>>();

                        let tool_result = json!({
                            "content": content,
                            "isError": false,
                        });

                        let response = SubsystemRpcResponse {
                            response: QueryResponse {
                                body: QueryResponseBody::Json(tool_result),
                                headers: extra_headers,
                            },
                            status_code,
                        };

                        Ok(Some(response))
                    }
                    None => Err(SubsystemRpcError::MethodNotFound(name.to_string())),
                }
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

    // This is to get around a bug in some MCP clients (Claude, for example) that try to get prompts and resources from the server even
    // though we declared to not support those in initialize.
    async fn handle_prompts_resources_list(
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
        headers.insert("content-type".into(), "application/json".into());

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
                                    self.handle_prompts_resources_list(request, request_context)
                                        .await
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

        let status_code = match &response {
            Ok(Some(response)) => response.status_code,
            Ok(None) => StatusCode::OK,
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

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
            status_code,
        })
    }
}
