use std::sync::Arc;

use async_trait::async_trait;

use async_stream::try_stream;
use bytes::Bytes;

use common::{
    context::RequestContext,
    env_const::get_rpc_http_path,
    http::{Headers, RequestHead, ResponseBody, ResponsePayload},
    router::Router,
};
use core_resolver::{
    QueryResponse, QueryResponseBody,
    plugin::subsystem_rpc_resolver::{
        JsonRpcId, JsonRpcRequest, SubsystemRpcError, SubsystemRpcResponse,
    },
    system_rpc_resolver::SystemRpcResolver,
};
use exo_env::Environment;
use http::StatusCode;
use rpc_introspection::{OpenRpcDocument, RpcSchema, to_openrpc};

const OPENRPC_API_TITLE: &str = "Exograph RPC API";
const OPENRPC_API_VERSION: &str = "1.0.0";

pub struct RpcRouter {
    system_resolver: SystemRpcResolver,
    api_path_prefix: String,
    discover_path: String,
}

/// The JSON-RPC method name for discovery
const RPC_DISCOVER_METHOD: &str = "rpc.discover";

impl RpcRouter {
    pub fn new(system_resolver: SystemRpcResolver, env: Arc<dyn Environment>) -> Self {
        let api_path_prefix = get_rpc_http_path(env.as_ref()).clone();
        let discover_path = format!("{}/discover", api_path_prefix);
        Self {
            system_resolver,
            api_path_prefix,
            discover_path,
        }
    }

    /// Check if the request path matches the RPC endpoint
    fn is_rpc_request(&self, request_head: &(dyn RequestHead + Sync)) -> bool {
        request_head.get_path() == self.api_path_prefix
    }

    /// Check if this is a GET request to /rpc/discover
    fn is_discover_request(&self, request_head: &(dyn RequestHead + Sync)) -> bool {
        request_head.get_path() == self.discover_path && request_head.get_method() == "GET"
    }

    /// Build the OpenRPC document from all subsystem schemas
    fn build_openrpc_document(&self) -> OpenRpcDocument {
        let mut combined = RpcSchema::new();
        for schema in self.system_resolver.rpc_schemas() {
            combined.merge(schema);
        }
        to_openrpc(&combined, OPENRPC_API_TITLE, OPENRPC_API_VERSION)
    }

    /// Handle the discover endpoint (GET /rpc/discover)
    fn handle_discover(&self) -> ResponsePayload {
        let openrpc_doc = self.build_openrpc_document();
        let body = serde_json::to_string_pretty(&openrpc_doc).unwrap_or_else(|_| "{}".to_string());

        let mut headers = Headers::new();
        headers.insert("content-type".into(), "application/json".into());

        ResponsePayload {
            body: ResponseBody::Bytes(body.into_bytes()),
            headers,
            status_code: StatusCode::OK,
        }
    }
}

const ERROR_METHOD_NOT_FOUND_CODE: &str = "-32601";
const ERROR_METHOD_NOT_FOUND_MESSAGE: &str = "Method not found";

#[async_trait]
impl<'a> Router<RequestContext<'a>> for RpcRouter {
    async fn route(&self, request_context: &RequestContext<'a>) -> Option<ResponsePayload> {
        // Handle GET /rpc/discover
        if self.is_discover_request(request_context.get_head()) {
            return Some(self.handle_discover());
        }

        // Handle regular RPC requests
        if !self.is_rpc_request(request_context.get_head()) {
            return None;
        }

        use common::http::RequestPayload;

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
                        id = request.id;

                        // Handle rpc.discover method
                        if request.method == RPC_DISCOVER_METHOD {
                            match serde_json::to_value(self.build_openrpc_document()) {
                                Ok(openrpc_json) => Ok(Some(SubsystemRpcResponse {
                                    response: QueryResponse {
                                        body: QueryResponseBody::Json(openrpc_json),
                                        headers: vec![],
                                    },
                                    status_code: StatusCode::OK,
                                })),
                                Err(e) => {
                                    tracing::error!(
                                        "Failed to serialize OpenRPC document: {:?}",
                                        e
                                    );
                                    Err(SubsystemRpcError::InternalError)
                                }
                            }
                        } else {
                            self.system_resolver
                                .resolve(&request.method, &request.params, request_context)
                                .await
                        }
                    }
                }
                Err(_) => Err(SubsystemRpcError::ParseError),
            }
        };

        // Copy headers from response if available
        if let Ok(Some(ref response)) = response {
            for (key, value) in response.response.headers.iter() {
                headers.insert(key.into(), value.into());
            }
        }

        let stream = try_stream! {
            macro_rules! emit_jsonrpc_id_and_close {
                () => {
                    yield Bytes::from_static(br#", "jsonrpc": "2.0", "id": "#);

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
                            yield Bytes::from_static(br#"null"#);
                        }
                    };

                    yield Bytes::from_static(br#"}"#);
                };
            }

            match response {
                Ok(Some(response)) => {
                    yield Bytes::from_static(br#"{"result": "#);

                    match response.response.body {
                        QueryResponseBody::Json(value) => yield Bytes::from(value.to_string()),
                        QueryResponseBody::Raw(Some(value)) => yield Bytes::from(value),
                        QueryResponseBody::Raw(None) => yield Bytes::from_static(b"null"),
                    };

                    emit_jsonrpc_id_and_close!();
                },
                Ok(None) => {
                    yield Bytes::from_static(br#"{"error": {"code": "#);
                    yield Bytes::from_static(ERROR_METHOD_NOT_FOUND_CODE.as_bytes());
                    yield Bytes::from_static(br#", "message": ""#);
                    yield Bytes::from_static(ERROR_METHOD_NOT_FOUND_MESSAGE.as_bytes());
                    yield Bytes::from_static(br#""}"#);
                    emit_jsonrpc_id_and_close!();
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
                    emit_jsonrpc_id_and_close!();
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
