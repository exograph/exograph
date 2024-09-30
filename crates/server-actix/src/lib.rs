// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

mod request;

use std::sync::Arc;

use actix_web::{
    web::{self, ServiceConfig},
    HttpRequest, HttpResponse, HttpResponseBuilder, Responder,
};
use exo_env::Environment;
use reqwest::StatusCode;
use url::Url;

use common::{
    env_const::{get_deployment_mode, DeploymentMode},
    http::RedirectType,
    router::Router,
};
use common::{
    http::{RequestHead, RequestPayload, ResponseBody, ResponsePayload},
    router::CompositeRouter,
};
use request::ActixRequestHead;
use serde_json::Value;

macro_rules! error_msg {
    ($msg:literal) => {
        concat!("{\"errors\": [{\"message\":\"", $msg, "\"}]}").as_bytes()
    };
}

pub fn configure_router(
    system_router: web::Data<CompositeRouter>,
    env: Arc<dyn Environment>,
) -> impl FnOnce(&mut ServiceConfig) {
    let endpoint_url = match get_deployment_mode(env.as_ref()) {
        Ok(DeploymentMode::Playground(url)) => Some(Url::parse(&url).unwrap()),
        _ => None,
    };

    move |app| {
        app.app_data(system_router)
            .app_data(web::Data::new(endpoint_url))
            .default_service(web::to(resolve));
    }
}

/// Resolve a GraphQL request
///
/// # Arguments
/// * `endpoint_url` - The target URL for resolving data (None implies that the current server is also the target)
async fn resolve(
    http_request: HttpRequest,
    body: Option<web::Json<Value>>,
    query: web::Query<Value>,
    endpoint_url: web::Data<Option<Url>>,
    system_router: web::Data<CompositeRouter>,
) -> impl Responder {
    match endpoint_url.as_ref() {
        Some(endpoint_url) => match http_request.headers().get("_exo_operation_kind") {
            Some(value) if value == "schema_query" => {
                // This is a schema fetch request, so solve it locally
                resolve_locally(http_request, body, query.into_inner(), system_router).await
            }
            _ => forward_request(http_request, body, endpoint_url).await,
        },
        None => {
            // We aren't operating in the playground mode, so we can resolve it here
            resolve_locally(http_request, body, query.into_inner(), system_router).await
        }
    }
}

struct ActixRequestPayload {
    head: ActixRequestHead,
    body: Value,
}

impl RequestPayload for ActixRequestPayload {
    fn get_head(&self) -> &(dyn RequestHead + Send + Sync) {
        &self.head
    }

    fn take_body(&mut self) -> Value {
        self.body.take()
    }
}

async fn resolve_locally(
    req: HttpRequest,
    body: Option<web::Json<Value>>,
    query: Value,
    system_router: web::Data<CompositeRouter>,
) -> HttpResponse {
    let playground_request = req
        .headers()
        .get("_exo_playground")
        .map(|value| value == "true")
        .unwrap_or(false);

    let mut request = ActixRequestPayload {
        head: ActixRequestHead::from_request(req, query),
        body: body.map(|b| b.into_inner()).unwrap_or(Value::Null),
    };

    let response = system_router.route(&mut request, playground_request).await;

    match response {
        Some(ResponsePayload {
            body,
            headers,
            status_code,
        }) => {
            let mut builder = HttpResponse::build(status_code);

            for header in headers.into_iter() {
                builder.append_header(header);
            }

            match body {
                ResponseBody::Stream(stream) => builder.streaming(stream),
                ResponseBody::Bytes(bytes) => builder.body(bytes),
                ResponseBody::Redirect(url, redirect_type) => {
                    let status = match redirect_type {
                        RedirectType::Temporary => StatusCode::TEMPORARY_REDIRECT,
                        RedirectType::Permanent => StatusCode::PERMANENT_REDIRECT,
                    };

                    HttpResponse::build(status)
                        .append_header(("Location", url))
                        .body("")
                }
                ResponseBody::None => builder.body(""),
            }
        }
        None => HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR)
            .body(error_msg!("Error resolving request")),
    }
}

async fn forward_request(
    req: HttpRequest,
    body: Option<web::Json<Value>>,
    forward_url: &Url,
) -> HttpResponse {
    let mut forward_url = forward_url.clone();
    forward_url.set_query(req.uri().query());

    let body = body
        .map(|b| b.into_inner().to_string())
        .unwrap_or("".to_string());

    let forwarded_req = reqwest::Client::default()
        .request(req.method().clone(), forward_url)
        .body(body);

    let forwarded_req = req
        .headers()
        .iter()
        .filter(|(h, _)| *h != "origin" && *h != "connection" && *h != "host")
        .fold(forwarded_req, |forwarded_req, (h, v)| {
            forwarded_req.header(h.as_str(), v.as_bytes())
        });

    let res = match forwarded_req.send().await {
        Ok(res) => res,
        Err(err) => {
            tracing::error!("Error forwarding request to the endpoint: {}", err);
            return HttpResponse::InternalServerError()
                .body(error_msg!("Error forwarding request to the endpoint"));
        }
    };

    let mut client_resp = HttpResponseBuilder::new(res.status());

    for (header_name, header_value) in res.headers().iter().filter(|(h, _)| *h != "connection") {
        client_resp.insert_header((header_name.clone(), header_value.clone()));
    }

    match res.bytes().await {
        Ok(bytes) => client_resp.body(bytes),
        Err(err) => {
            tracing::error!("Error reading response body from endpoint: {}", err);
            client_resp.body(error_msg!("Error reading response body from endpoint"))
        }
    }
}
