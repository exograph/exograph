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
    QueryResponseBody,
    plugin::subsystem_rpc_resolver::{JsonRpcId, JsonRpcRequest, SubsystemRpcError},
    system_rpc_resolver::SystemRpcResolver,
};
use exo_env::Environment;
use http::StatusCode;

pub struct RpcRouter {
    system_resolver: SystemRpcResolver,
    api_path_prefix: String,
}

impl RpcRouter {
    pub fn new(system_resolver: SystemRpcResolver, env: Arc<dyn Environment>) -> Self {
        Self {
            system_resolver,
            api_path_prefix: get_rpc_http_path(env.as_ref()).clone(),
        }
    }

    fn suitable(&self, request_head: &(dyn RequestHead + Sync)) -> bool {
        request_head.get_path() == self.api_path_prefix
    }
}

const ERROR_METHOD_NOT_FOUND_CODE: &str = "-32601";
const ERROR_METHOD_NOT_FOUND_MESSAGE: &str = "Method not found";

#[async_trait]
impl<'a> Router<RequestContext<'a>> for RpcRouter {
    async fn route(&self, request_context: &RequestContext<'a>) -> Option<ResponsePayload> {
        if !self.suitable(request_context.get_head()) {
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
                        let response = self
                            .system_resolver
                            .resolve(&request.method, &request.params, request_context)
                            .await;

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
