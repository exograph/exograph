use std::sync::Arc;

use async_graphql_parser::types::TypeKind;
use async_trait::async_trait;

use async_stream::try_stream;
use bytes::Bytes;

use common::{
    context::RequestContext,
    env_const::{get_mcp_http_path, EXO_WWW_AUTHENTICATE_HEADER},
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
use serde_json::{json, Map, Value};

const ERROR_METHOD_NOT_FOUND_CODE: &str = "-32601";
const ERROR_METHOD_NOT_FOUND_MESSAGE: &str = "Method not found";

const WWW_AUTHENTICATE_HEADER: &str = "WWW-Authenticate";

/// MCP router
///
/// Partially supports the new [Streamable HTTP](https://spec.modelcontextprotocol.io/specification/2025-03-26/basic/transports/#streamable-http)
/// protocol. Once this specification is finalized and the [official SDK](https://github.com/modelcontextprotocol/rust-sdk) supports it, we will
/// use types from that crate.
///
/// The implementation forwards requests to the GraphQL resolver.
pub struct McpRouter {
    api_path_prefix: String,
    data_resolver: Arc<GraphQLSystemResolver>,
    introspection_resolver: Arc<GraphQLSystemResolver>,
    www_authenticate_header: Option<String>,
    tool_mode: McpToolMode,
}

#[derive(Debug, PartialEq, Eq)]
enum McpToolMode {
    CombineIntrospection,
    SeparateIntrospection,
}

impl McpRouter {
    pub fn new(
        env: Arc<dyn Environment>,
        data_resolver: Arc<GraphQLSystemResolver>,
        introspection_resolver: Arc<GraphQLSystemResolver>,
    ) -> Self {
        let www_authenticate_header = env.get(EXO_WWW_AUTHENTICATE_HEADER);

        let tool_mode = if env.get_or_else("EXO_UNSTABLE_MCP_MODE", "combine") == "separate" {
            McpToolMode::SeparateIntrospection
        } else {
            McpToolMode::CombineIntrospection
        };

        Self {
            api_path_prefix: get_mcp_http_path(env.as_ref()).clone(),
            data_resolver,
            introspection_resolver,
            www_authenticate_header,
            tool_mode,
        }
    }

    fn suitable(&self, request_head: &(dyn RequestHead + Sync)) -> bool {
        let method = request_head.get_method();

        request_head.get_path().starts_with(&self.api_path_prefix)
            && (method == Method::GET || method == Method::POST)
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

        let response_body = if self.tool_mode == McpToolMode::CombineIntrospection {
            json!({
                "tools": [{
                    "name": "execute_query",
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
            })
        } else {
            let entities = self
                .data_resolver
                .schema
                .type_definitions
                .iter()
                .filter_map(|td| match td.kind {
                    TypeKind::Object(_) if !td.name.node.starts_with("__") => {
                        Some(td.name.node.to_string())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(", ");
            let schema_description = self
                .data_resolver
                .schema
                .declaration_doc_comments
                .as_deref()
                .unwrap_or_default();
            json!({
                "tools": [{
                    "name": "execute_query",
                    "description": format!(r#"
                        Execute a GraphQL query per the schema obtained through the `introspect` tool.
                        Before executing a query, you must invoke the `introspect` tool to get the queries and their arguments.

                        The schema supports querying the following entities: {entities}.
                        
                        {schema_description}
                        "#
                    ),
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
                }, {
                    "name": "introspect",
                    "description": "Introspect the GraphQL schema to get supported queries and their arguments.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {},
                        "additionalProperties": false
                    }
                }]
            })
        };

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

                match name {
                    "execute_query" => {
                        self.handle_execute_query_tools_call(request, request_context)
                            .await
                    }
                    "introspect" => {
                        self.handle_introspect_tools_call(request, request_context)
                            .await
                    }
                    _ => Err(SubsystemRpcError::MethodNotFound(name.to_string())),
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

    async fn handle_execute_query_tools_call(
        &self,
        request: JsonRpcRequest,
        request_context: &RequestContext<'_>,
    ) -> Result<Option<SubsystemRpcResponse>, SubsystemRpcError> {
        let payload: ExecuteQueryPayload =
            serde_json::from_value(request.params.unwrap_or_default())
                .map_err(|_| SubsystemRpcError::InvalidRequest)?;

        let arguments = payload.arguments;

        let graphql_response = execute_query(
            arguments.query,
            arguments.variables,
            &self.data_resolver,
            request_context,
        )
        .await;

        let (tool_result, status_code, extra_headers) = match graphql_response {
            Ok(graphql_response) => {
                let (response_contents, response_headers): (Vec<_>, Vec<_>) = graphql_response
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

                        (
                            json!({
                                "text": text,
                                "type": "text",
                            }),
                            response.headers,
                        )
                    })
                    .unzip();

                (
                    json!({
                        "content": response_contents,
                        "isError": false,
                    }),
                    StatusCode::OK,
                    response_headers.into_iter().flatten().collect(),
                )
            }
            Err(e) => {
                let authentication_present = request_context.is_authentication_info_present();

                let status_code = match e {
                    SubsystemRpcError::ExpiredAuthentication => StatusCode::UNAUTHORIZED,
                    SubsystemRpcError::Authorization => {
                        if authentication_present {
                            StatusCode::FORBIDDEN
                        } else {
                            StatusCode::UNAUTHORIZED
                        }
                    }
                    SubsystemRpcError::ParseError
                    | SubsystemRpcError::InvalidParams(_, _)
                    | SubsystemRpcError::InvalidRequest
                    | SubsystemRpcError::UserDisplayError(_) => StatusCode::BAD_REQUEST,
                    SubsystemRpcError::MethodNotFound(_) => StatusCode::NOT_FOUND,
                    SubsystemRpcError::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
                };

                let extra_headers: Vec<(String, String)> =
                    match (status_code, &self.www_authenticate_header) {
                        (StatusCode::UNAUTHORIZED, Some(www_authenticate_header)) => {
                            vec![(
                                WWW_AUTHENTICATE_HEADER.to_string(),
                                www_authenticate_header.clone(),
                            )]
                        }
                        _ => vec![],
                    };

                (
                    json!({
                        "content": [json!({
                            "text": e.user_error_message().unwrap_or_default(),
                            "type": "text",
                        })],
                        "isError": true,
                    }),
                    status_code,
                    extra_headers,
                )
            }
        };

        let response = SubsystemRpcResponse {
            response: QueryResponse {
                body: QueryResponseBody::Json(tool_result),
                headers: extra_headers,
            },
            status_code,
        };

        Ok(Some(response))
    }

    async fn handle_introspect_tools_call(
        &self,
        _request: JsonRpcRequest,
        request_context: &RequestContext<'_>,
    ) -> Result<Option<SubsystemRpcResponse>, SubsystemRpcError> {
        Ok(Some(SubsystemRpcResponse {
            response: QueryResponse {
                body: QueryResponseBody::Json(json!({
                    "content": [json!({
                        "text": self.get_introspection_schema(request_context).await?,
                        "type": "text",
                    })],
                    "isError": false,
                })),
                headers: vec![],
            },
            status_code: StatusCode::OK,
        }))
    }

    async fn get_introspection_schema(
        &self,
        request_context: &RequestContext<'_>,
    ) -> Result<String, SubsystemRpcError> {
        let query = introspection_util::get_introspection_query()
            .await
            .map_err(|_| SubsystemRpcError::InternalError)?;

        let query = query.as_str().ok_or(SubsystemRpcError::InternalError)?;

        let graphql_response = execute_query(
            query.to_string(),
            None,
            &self.introspection_resolver,
            request_context,
        )
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

#[derive(serde::Deserialize)]
struct ExecuteQueryPayload {
    #[allow(dead_code)]
    name: String,
    arguments: ExecuteQueryArguments,
}

#[derive(serde::Deserialize)]
struct ExecuteQueryArguments {
    query: String,
    variables: Option<Map<String, Value>>,
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

async fn execute_query(
    query: String,
    variables: Option<Map<String, Value>>,
    system_resolver: &GraphQLSystemResolver,
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
        system_resolver,
        TrustedDocumentEnforcement::DoNotEnforce,
        request_context,
    )
    .await
    .map_err(|e| {
        tracing::error!("Error while resolving request: {:?}", e);
        e.into()
    })
}
