use std::sync::Arc;

use async_stream::try_stream;
use async_trait::async_trait;
use bytes::Bytes;
use serde_json::Value;

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
    system_resolver::GraphQLSystemResolver,
    system_rpc_resolver::SystemRpcResolver,
};
use exo_env::Environment;
use http::StatusCode;
use rpc_introspection::{OpenRpcDocument, RpcSchema, SchemaGeneration, to_rpc_document};

const OPENRPC_API_TITLE: &str = "Exograph RPC API";
const OPENRPC_API_VERSION: &str = "1.0.0";

pub struct RpcRouter {
    system_resolver: SystemRpcResolver,
    graphql_system_resolver: Arc<GraphQLSystemResolver>,
    api_path_prefix: String,
    discover_path: String,
    openrpc_document: OpenRpcDocument,
}

/// The JSON-RPC method name for discovery
const RPC_DISCOVER_METHOD: &str = "rpc.discover";

const ERROR_METHOD_NOT_FOUND_CODE: &str = "-32601";
const ERROR_METHOD_NOT_FOUND_MESSAGE: &str = "Method not found";
const BATCH_ROLLBACK_MESSAGE: &str =
    "Transaction rolled back due to other failed request(s) in the batch";

/// Result of resolving a single JSON-RPC request
struct SingleRpcResult {
    id: Option<JsonRpcId>,
    outcome: Result<Option<SubsystemRpcResponse>, SubsystemRpcError>,
    is_notification: bool,
}

impl RpcRouter {
    pub fn new(
        system_resolver: SystemRpcResolver,
        graphql_system_resolver: Arc<GraphQLSystemResolver>,
        rpc_schema: Option<RpcSchema>,
        env: Arc<dyn Environment>,
    ) -> Self {
        let api_path_prefix = get_rpc_http_path(env.as_ref()).clone();
        let discover_path = format!("{}/discover", api_path_prefix);

        let rpc_document =
            to_rpc_document(&rpc_schema.unwrap_or_default(), SchemaGeneration::OpenRpc);
        let openrpc_document = OpenRpcDocument::new(OPENRPC_API_TITLE, OPENRPC_API_VERSION)
            .with_document(rpc_document);

        Self {
            system_resolver,
            graphql_system_resolver,
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

    /// Resolve a single JSON-RPC request into a structured result
    async fn resolve_single_request<'a>(
        &self,
        body: Value,
        request_context: &RequestContext<'a>,
    ) -> SingleRpcResult {
        let request: Result<JsonRpcRequest, _> =
            serde_json::from_value(body).map_err(|_| SubsystemRpcError::InvalidRequest);

        match request {
            Ok(request) => {
                if request.jsonrpc != "2.0" {
                    return SingleRpcResult {
                        id: request.id,
                        outcome: Err(SubsystemRpcError::InvalidRequest),
                        is_notification: false,
                    };
                }

                let id = request.id.clone();
                let is_notification = id.is_none();

                if is_notification {
                    return SingleRpcResult {
                        id: None,
                        outcome: Ok(None),
                        is_notification: true,
                    };
                }

                let outcome = if request.method == RPC_DISCOVER_METHOD {
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
                        .resolve(
                            &request.method,
                            &request.params,
                            request_context,
                            &self.graphql_system_resolver,
                        )
                        .await
                };

                SingleRpcResult {
                    id,
                    outcome,
                    is_notification,
                }
            }
            Err(err) => SingleRpcResult {
                id: None,
                outcome: Err(err),
                is_notification: false,
            },
        }
    }

    /// Write JSON-RPC response bytes into the provided buffer.
    /// Static fragments use `Bytes::from_static` (zero-copy); only dynamic
    /// data (result values, error messages, ids) allocates.
    fn write_rpc_response(result: SingleRpcResult, out: &mut Vec<Bytes>) {
        let SingleRpcResult { id, outcome, .. } = result;

        match outcome {
            Ok(Some(response)) => {
                out.push(Bytes::from_static(br#"{"result": "#));
                match response.response.body {
                    QueryResponseBody::Json(value) => {
                        out.push(Bytes::from(value.to_string()));
                    }
                    QueryResponseBody::Raw(Some(value)) => {
                        out.push(Bytes::from(value));
                    }
                    QueryResponseBody::Raw(None) => {
                        out.push(Bytes::from_static(b"null"));
                    }
                };
            }
            Ok(None) => {
                out.push(Bytes::from_static(br#"{"error": {"code": "#));
                out.push(Bytes::from_static(ERROR_METHOD_NOT_FOUND_CODE.as_bytes()));
                out.push(Bytes::from_static(br#", "message": ""#));
                out.push(Bytes::from_static(
                    ERROR_METHOD_NOT_FOUND_MESSAGE.as_bytes(),
                ));
                out.push(Bytes::from_static(br#""}"#));
            }
            Err(err) => {
                tracing::error!("Error while resolving request: {:?}", err);
                out.push(Bytes::from_static(br#"{"error": {"code": "#));
                out.push(Bytes::from_static(err.error_code_string().as_bytes()));
                out.push(Bytes::from_static(br#", "message": "#));
                let message = err.user_error_message().unwrap_or_default();
                out.push(Bytes::from(
                    serde_json::to_string(&message).unwrap_or_else(|_| "\"\"".to_string()),
                ));
                out.push(Bytes::from_static(br#"}"#));
            }
        }

        out.push(Bytes::from_static(br#", "jsonrpc": "2.0", "id": "#));
        match &id {
            Some(id_value) => {
                out.push(Bytes::from(
                    serde_json::to_string(id_value)
                        .expect("BUG: JsonRpcId serialization should not fail"),
                ));
            }
            None => {
                out.push(Bytes::from_static(b"null"));
            }
        }
        out.push(Bytes::from_static(br#"}"#));
    }

    /// Handle a single JSON-RPC request
    async fn handle_single<'a>(
        &self,
        body: Value,
        request_context: &RequestContext<'a>,
    ) -> ResponsePayload {
        let result = self.resolve_single_request(body, request_context).await;

        if result.is_notification {
            return ResponsePayload {
                body: ResponseBody::None,
                headers: Headers::new(),
                status_code: StatusCode::NO_CONTENT,
            };
        }

        // Finalize the transaction (commit on success, rollback on error)
        let commit = result.outcome.is_ok();
        if let Err(e) = request_context.finalize_transaction(commit).await {
            tracing::error!("Error while finalizing transaction: {:?}", e);
        }

        let mut headers = Headers::new();
        headers.insert("content-type".into(), "application/json".into());

        // Copy headers from response
        if let Ok(Some(ref response)) = result.outcome {
            for (key, value) in &response.response.headers {
                headers.insert(key.into(), value.into());
            }
        }

        ResponsePayload {
            body: Self::rpc_response_body(result),
            headers,
            status_code: StatusCode::OK,
        }
    }

    /// Collect a single JSON-RPC response into `ResponseBody::Bytes`.
    fn rpc_response_body(result: SingleRpcResult) -> ResponseBody {
        let mut chunks = Vec::new();
        Self::write_rpc_response(result, &mut chunks);
        let total_len: usize = chunks.iter().map(|c| c.len()).sum();
        let mut buf = Vec::with_capacity(total_len);
        for chunk in chunks {
            buf.extend_from_slice(&chunk);
        }
        ResponseBody::Bytes(buf)
    }

    /// Handle a JSON-RPC batch request (array of requests).
    ///
    /// All requests in a batch share a single database transaction.
    /// If any request fails, the entire transaction rolls back.
    async fn handle_batch<'a>(
        &self,
        items: Vec<Value>,
        request_context: &RequestContext<'a>,
    ) -> ResponsePayload {
        let mut headers = Headers::new();
        headers.insert("content-type".into(), "application/json".into());

        // Empty array is invalid per JSON-RPC 2.0 spec (Section 6)
        if items.is_empty() {
            let error_result = SingleRpcResult {
                id: None,
                outcome: Err(SubsystemRpcError::InvalidRequest),
                is_notification: false,
            };
            return ResponsePayload {
                body: Self::rpc_response_body(error_result),
                headers,
                status_code: StatusCode::OK,
            };
        }

        // Ensure a shared transaction for all batch items
        request_context.ensure_transaction().await;

        let mut results = Vec::with_capacity(items.len());
        let mut all_succeeded = true;

        for item in items {
            let result = self.resolve_single_request(item, request_context).await;

            // A result is a failure if it's an explicit error OR method-not-found (Ok(None))
            // Notifications (Ok(None) with is_notification) are not failures
            let is_success = result.is_notification || matches!(result.outcome, Ok(Some(_)));
            if !is_success {
                all_succeeded = false;
            }

            // Collect response headers
            if let Ok(Some(ref response)) = result.outcome {
                for (key, value) in &response.response.headers {
                    headers.insert(key.into(), value.into());
                }
            }

            results.push(result);
        }

        // Finalize the transaction (commit only if all succeeded, rollback otherwise)
        if let Err(e) = request_context.finalize_transaction(all_succeeded).await {
            tracing::error!("Error while finalizing transaction: {:?}", e);
        }

        // Replace successful results with rollback errors since the transaction was rolled back
        if !all_succeeded {
            for result in &mut results {
                if matches!(result.outcome, Ok(Some(_))) {
                    result.outcome = Err(SubsystemRpcError::UserDisplayError(
                        BATCH_ROLLBACK_MESSAGE.to_string(),
                    ));
                }
            }
        }

        // Filter out notifications (per spec, notifications produce no response)
        let non_notification_results: Vec<_> =
            results.into_iter().filter(|r| !r.is_notification).collect();

        if non_notification_results.is_empty() {
            return ResponsePayload {
                body: ResponseBody::None,
                headers: Headers::new(),
                status_code: StatusCode::NO_CONTENT,
            };
        }

        // Stream the batch response as a JSON array
        let stream = try_stream! {
            yield Bytes::from_static(b"[");
            let mut chunks = Vec::new();
            for (i, result) in non_notification_results.into_iter().enumerate() {
                if i > 0 {
                    yield Bytes::from_static(b",");
                }
                Self::write_rpc_response(result, &mut chunks);
                for chunk in chunks.drain(..) {
                    yield chunk;
                }
            }
            yield Bytes::from_static(b"]");
        };

        ResponsePayload {
            body: ResponseBody::Stream(Box::pin(stream)),
            headers,
            status_code: StatusCode::OK,
        }
    }
}

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

        Some(match body {
            Value::Array(items) => self.handle_batch(items, request_context).await,
            body => self.handle_single(body, request_context).await,
        })
    }
}
