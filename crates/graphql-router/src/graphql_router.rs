// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::sync::Arc;

use async_trait::async_trait;
use common::env_const::get_graphql_http_path;

use common::env_const::is_production;
use common::http::{Headers, RequestHead, RequestPayload, ResponseBody, ResponsePayload};
use common::router::Router;
use core_plugin_shared::interception::InterceptionMap;
use core_plugin_shared::trusted_documents::TrustedDocumentEnforcement;
use core_plugin_shared::trusted_documents::TrustedDocuments;
use core_resolver::plugin::SubsystemGraphQLResolver;
use core_resolver::QueryResponse;
use core_router::SystemLoadingError;
use http::StatusCode;

use ::tracing::instrument;
use async_graphql_parser::Pos;
use async_stream::try_stream;
use bytes::Bytes;
use common::context::RequestContext;
use common::operation_payload::OperationsPayload;
use core_resolver::system_resolver::GraphQLSystemResolver;
use core_resolver::system_resolver::{RequestError, SystemResolutionError};
use core_resolver::QueryResponseBody;

use exo_env::Environment;

use crate::system_loader::SystemLoader;

pub struct GraphQLRouter {
    system_resolver: GraphQLSystemResolver,
    env: Arc<dyn Environment>,
}

impl GraphQLRouter {
    pub fn new(system_resolver: GraphQLSystemResolver, env: Arc<dyn Environment>) -> Self {
        Self {
            system_resolver,
            env,
        }
    }

    fn suitable(&self, request_head: &(dyn RequestHead + Sync)) -> bool {
        request_head.get_path() == get_graphql_http_path(self.env.as_ref())
            && request_head.get_method() == http::Method::POST
    }

    pub async fn from_resolvers(
        graphql_resolvers: Vec<Box<dyn SubsystemGraphQLResolver + Send + Sync>>,
        query_interception_map: InterceptionMap,
        mutation_interception_map: InterceptionMap,
        trusted_documents: TrustedDocuments,
        env: Arc<dyn Environment>,
    ) -> Result<Self, SystemLoadingError> {
        let graphql_resolver = SystemLoader::create_system_resolver(
            graphql_resolvers,
            query_interception_map,
            mutation_interception_map,
            trusted_documents,
            env.clone(),
        )
        .await?;

        Ok(Self::new(graphql_resolver, env))
    }
}

#[async_trait]
impl<'a> Router<RequestContext<'a>> for GraphQLRouter {
    /// Resolves an incoming query, returning a response stream containing JSON and a set
    /// of HTTP headers. The JSON may be either the data returned by the query, or a list of errors
    /// if something went wrong.
    ///
    /// In a typical use case (for example server-actix), the caller will
    /// first call `create_system_resolver_or_exit` to create a [SystemResolver] object, and
    /// then call `resolve` with that object.
    #[instrument(
        name = "resolver::resolve"
        skip(self, request_context)
    )]
    async fn route(&self, request_context: &mut RequestContext<'a>) -> Option<ResponsePayload> {
        let request_head = request_context.get_head();
        if !self.suitable(request_head) {
            return None;
        }

        let playground_request = request_head
            .get_header("_exo_playground")
            .map(|value| value == "true")
            .unwrap_or(false);

        let is_production = is_production(self.env.as_ref());

        // If the server is in production mode, enforce trusted documents regardless of
        // the `_exo_playground` header
        let trusted_document_enforcement = if is_production || !playground_request {
            TrustedDocumentEnforcement::Enforce
        } else {
            TrustedDocumentEnforcement::DoNotEnforce
        };

        let response = resolve_in_memory(
            request_context,
            &self.system_resolver,
            trusted_document_enforcement,
            &request_context,
        )
        .await;

        if let Err(SystemResolutionError::RequestError(e)) = response {
            tracing::error!("Error while resolving request: {:?}", e);
            return Some(ResponsePayload {
                body: ResponseBody::None,
                headers: Headers::new(),
                status_code: StatusCode::BAD_REQUEST,
            });
        }

        let mut headers = if let Ok(ref response) = response {
            Headers::from_vec(
                response
                    .iter()
                    .flat_map(|(_, qr)| qr.headers.clone())
                    .collect(),
            )
        } else {
            Headers::new()
        };

        headers.insert("content-type".into(), "application/json".into());

        let stream = try_stream! {
            macro_rules! report_position {
                ($position:expr) => {
                    let p: Pos = $position;

                    yield Bytes::from_static(br#"{"line": "#);
                    yield Bytes::from(p.line.to_string());
                    yield Bytes::from_static(br#", "column": "#);
                    yield Bytes::from(p.column.to_string());
                    yield Bytes::from_static(br#"}"#);
                };
            }

            macro_rules! report_positions {
                ($positions:expr) => {
                    let mut first = true;
                    for p in $positions {
                        if !first {
                            yield Bytes::from_static(b", ");
                        }
                        first = false;
                        report_position!(p);
                    }
                };
            }

            match response {
                Ok(parts) => {
                    let parts_len = parts.len();
                    yield Bytes::from_static(br#"{"data": {"#);
                    for (index, part) in parts.into_iter().enumerate() {
                        yield Bytes::from_static(b"\"");
                        yield Bytes::from(part.0);
                        yield Bytes::from_static(br#"":"#);
                        match part.1.body {
                            QueryResponseBody::Json(value) => yield Bytes::from(value.to_string()),
                            QueryResponseBody::Raw(Some(value)) => yield Bytes::from(value),
                            QueryResponseBody::Raw(None) => yield Bytes::from_static(b"null"),
                        };
                        if index != parts_len - 1 {
                            yield Bytes::from_static(b", ");
                        }
                    };
                    yield Bytes::from_static(b"}}");
                },
                Err(err) => {
                    yield Bytes::from_static(br#"{"errors": [{"message":""#);
                    yield Bytes::from(
                        err.user_error_message().to_string()
                            .replace('\"', "")
                            .replace('\n', "; ")
                    );
                    yield Bytes::from_static(br#"""#);
                    if let SystemResolutionError::Validation(err) = err {
                        yield Bytes::from_static(br#", "locations": ["#);
                        report_positions!(err.positions());
                        yield Bytes::from_static(br#"]"#);
                    };
                    yield Bytes::from_static(br#"}"#);
                    yield Bytes::from_static(b"]}");
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

#[instrument(
    name = "resolver::resolve_in_memory"
    skip(system_resolver, request, request_context)
)]
async fn resolve_in_memory<'a>(
    request: &(dyn RequestPayload + Sync),
    system_resolver: &GraphQLSystemResolver,
    trusted_document_enforcement: TrustedDocumentEnforcement,
    request_context: &RequestContext<'a>,
) -> Result<Vec<(String, QueryResponse)>, SystemResolutionError> {
    let body = request.take_body();

    let operations_payload = OperationsPayload::from_json(body.clone())
        .map_err(|e| SystemResolutionError::RequestError(RequestError::InvalidBodyJson(e)))?;

    let response = system_resolver
        .resolve_operations(
            operations_payload,
            request_context,
            trusted_document_enforcement,
        )
        .await;

    let ctx = request_context.get_base_context();
    let mut tx_holder = ctx.transaction_holder.try_lock().unwrap();

    tx_holder
        .finalize(response.is_ok())
        .await
        .map_err(|e| {
            SystemResolutionError::Generic(format!("Error while finalizing transaction: {e}"))
        })
        .and(response)
}
