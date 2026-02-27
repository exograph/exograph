use std::sync::Arc;

use async_graphql_parser::types::TypeKind;
use async_trait::async_trait;
use common::context::RequestContext;
use core_resolver::{
    QueryResponseBody,
    plugin::subsystem_rpc_resolver::{JsonRpcRequest, SubsystemRpcError},
    system_resolver::GraphQLSystemResolver,
};
use http::StatusCode;
use serde_json::{Map, Value, json};

use crate::{executor::Executor, tool::Tool, tools_creator::McpToolMode};

const WWW_AUTHENTICATE_HEADER: &str = "WWW-Authenticate";

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

pub struct ExecuteQueryTool {
    name: String,
    resolver: Arc<GraphQLSystemResolver>,
    www_authenticate_header: Option<String>,
    tool_mode: McpToolMode,
}

impl ExecuteQueryTool {
    pub fn new(
        name: String,
        resolver: Arc<GraphQLSystemResolver>,
        www_authenticate_header: Option<String>,
        tool_mode: McpToolMode,
    ) -> Self {
        Self {
            name,
            resolver,
            www_authenticate_header,
            tool_mode,
        }
    }
}

#[async_trait]
impl Tool for ExecuteQueryTool {
    fn name(&self) -> String {
        self.name.clone()
    }

    async fn description(
        &self,
        request_context: &RequestContext<'_>,
    ) -> Result<String, SubsystemRpcError> {
        match self.tool_mode {
            McpToolMode::CombineIntrospection => {
                let introspection_schema = self
                    .resolver
                    .get_introspection_schema(request_context)
                    .await?;

                Ok(format!(
                    "Execute a GraphQL query per the following schema: {introspection_schema}",
                ))
            }
            McpToolMode::SeparateIntrospection => {
                let entities = self
                    .resolver
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
                    .resolver
                    .schema
                    .declaration_doc_comments
                    .as_deref()
                    .unwrap_or_default();

                Ok(format!(
                    r#"
                Execute a GraphQL query per the schema obtained through the `introspect` tool.
                Before executing a query, you must invoke the `introspect` tool to get the queries and their arguments.

                The schema supports querying the following entities: {entities}.
                
                {schema_description}
                "#
                ))
            }
        }
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
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
        })
    }

    async fn execute(
        &self,
        request: JsonRpcRequest,
        request_context: &RequestContext<'_>,
    ) -> Result<(Vec<String>, StatusCode, Vec<(String, String)>), SubsystemRpcError> {
        let payload: ExecuteQueryPayload =
            serde_json::from_value(request.params.unwrap_or_default())
                .map_err(|_| SubsystemRpcError::InvalidRequest)?;

        let arguments = payload.arguments;

        let graphql_response = self
            .resolver
            .execute_query(arguments.query, arguments.variables, request_context)
            .await;

        match graphql_response {
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

                        (text, response.headers)
                    })
                    .unzip();

                Ok((
                    response_contents,
                    StatusCode::OK,
                    response_headers.into_iter().flatten().collect(),
                ))
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
                    | SubsystemRpcError::InvalidParams(_)
                    | SubsystemRpcError::InvalidRequest
                    | SubsystemRpcError::UserDisplayError(_)
                    | SubsystemRpcError::SystemResolutionError(_)
                    | SubsystemRpcError::Other(_) => StatusCode::BAD_REQUEST,
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

                Ok((
                    vec![format!(
                        "Error: {}",
                        e.user_error_message().unwrap_or_default()
                    )],
                    status_code,
                    extra_headers,
                ))
            }
        }
    }
}
