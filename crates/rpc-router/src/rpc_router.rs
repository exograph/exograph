use std::sync::Arc;

use async_stream::try_stream;
use async_trait::async_trait;
use bytes::Bytes;

use common::{
    context::RequestContext,
    env_const::get_rpc_http_path,
    http::{Headers, RequestHead, ResponseBody, ResponsePayload},
    router::Router,
};
use core_resolver::{
    QueryResponse, QueryResponseBody,
    plugin::subsystem_rpc_resolver::{JsonRpcRequest, SubsystemRpcError, SubsystemRpcResponse},
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
    openrpc_document: OpenRpcDocument,
}

/// The JSON-RPC method name for discovery
const RPC_DISCOVER_METHOD: &str = "rpc.discover";

impl RpcRouter {
    pub fn new(system_resolver: SystemRpcResolver, env: Arc<dyn Environment>) -> Self {
        let api_path_prefix = get_rpc_http_path(env.as_ref()).clone();
        let discover_path = format!("{}/discover", api_path_prefix);

        let mut combined = RpcSchema::new();
        for schema in system_resolver.rpc_schemas() {
            combined.merge(schema.clone());
        }
        let openrpc_document = to_openrpc(&combined, OPENRPC_API_TITLE, OPENRPC_API_VERSION);

        Self {
            system_resolver,
            api_path_prefix,
            discover_path,
            openrpc_document,
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

    /// Handle the discover endpoint (GET /rpc/discover)
    fn handle_discover(&self) -> ResponsePayload {
        let body = serde_json::to_string_pretty(&self.openrpc_document)
            .unwrap_or_else(|_| "{}".to_string());

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
            serde_json::from_value(body).map_err(|_| SubsystemRpcError::InvalidRequest);

        let mut id = None;
        let mut headers = Headers::new();
        headers.insert("content-type".into(), "application/json".into());

        let response = match request {
            Ok(request) => {
                if request.jsonrpc != "2.0" {
                    Err(SubsystemRpcError::InvalidRequest)
                } else {
                    id = request.id;

                    if id.is_none() {
                        // notification - no response needed
                        return Some(ResponsePayload {
                            body: ResponseBody::None,
                            headers: Headers::new(),
                            status_code: StatusCode::NO_CONTENT,
                        });
                    }

                    if request.method == RPC_DISCOVER_METHOD {
                        match serde_json::to_value(&self.openrpc_document) {
                            Ok(openrpc_json) => Ok(Some(SubsystemRpcResponse {
                                response: QueryResponse {
                                    body: QueryResponseBody::Json(openrpc_json),
                                    headers: vec![],
                                },
                                status_code: StatusCode::OK,
                            })),
                            Err(e) => {
                                tracing::error!("Failed to serialize OpenRPC document: {:?}", e);
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
            Err(err) => Err(err),
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
                    match &id {
                        Some(id_value) => {
                            let serialized_id = serde_json::to_string(id_value).expect("BUG: JsonRpcId serialization should not fail");
                            yield Bytes::from(serialized_id);
                        }
                        None => {
                            yield Bytes::from_static(br#"null"#);
                        }
                    }
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
                    yield Bytes::from_static(br#", "message": "#);
                    let message = err.user_error_message().unwrap_or_default();
                    yield Bytes::from(serde_json::to_string(&message).unwrap_or_else(|_| "\"\"".to_string()));
                    yield Bytes::from_static(br#"}"#);
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
