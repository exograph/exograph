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
use std::sync::Mutex;

use actix_web::{
    HttpRequest, HttpResponse, HttpResponseBuilder, Responder,
    web::{self, ServiceConfig},
};
use exo_env::Environment;
use reqwest::StatusCode;
use system_router::SystemRouter;
use url::Url;

use common::{
    env_const::{DeploymentMode, get_deployment_mode},
    router::Router,
};
use common::{
    http::{RequestHead, RequestPayload, ResponseBody, ResponsePayload},
    router::PlainRequestPayload,
};
use request::ActixRequestHead;
use serde_json::Value;

macro_rules! error_msg {
    ($msg:literal) => {
        concat!("{\"errors\": [{\"message\":\"", $msg, "\"}]}").as_bytes()
    };
}

pub fn configure_router(
    system_router: web::Data<SystemRouter>,
    env: Arc<dyn Environment>,
) -> impl FnOnce(&mut ServiceConfig) {
    let endpoint_url = match get_deployment_mode(env.as_ref()) {
        Ok(Some(DeploymentMode::Playground(url))) => Some(Url::parse(&url).unwrap()),
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
    system_router: web::Data<SystemRouter>,
) -> impl Responder {
    match endpoint_url.as_ref() {
        Some(endpoint_url) => {
            // In the playground mode, locally serve the schema query or playground assets
            let schema_query = http_request
                .headers()
                .get("_exo_operation_kind")
                .map(|v| v.as_bytes())
                == Some(b"schema_query");

            if schema_query
                || system_router.is_playground_assets_request(
                    http_request.path(),
                    to_reqwest_method(http_request.method()),
                )
            {
                resolve_locally(http_request, body, query.into_inner(), system_router).await
            } else {
                forward_request(http_request, body, endpoint_url).await
            }
        }
        None => {
            // We aren't operating in the playground mode, so we can resolve it locally
            resolve_locally(http_request, body, query.into_inner(), system_router).await
        }
    }
}

struct ActixRequestPayload {
    head: ActixRequestHead,
    body: Mutex<Value>,
}

impl RequestPayload for ActixRequestPayload {
    fn get_head(&self) -> &(dyn RequestHead + Send + Sync) {
        &self.head
    }

    fn take_body(&self) -> Value {
        self.body.lock().unwrap().take()
    }
}

async fn resolve_locally(
    req: HttpRequest,
    body: Option<web::Json<Value>>,
    query: Value,
    system_router: web::Data<SystemRouter>,
) -> HttpResponse {
    let request = ActixRequestPayload {
        head: ActixRequestHead::from_request(req, query),
        body: Mutex::new(body.map(|b| b.into_inner()).unwrap_or(Value::Null)),
    };

    let response = system_router
        .route(&PlainRequestPayload::external(Box::new(request)))
        .await;

    match response {
        Some(ResponsePayload {
            body,
            headers,
            status_code,
        }) => {
            let actix_status_code = match to_actix_status_code(status_code) {
                Ok(status_code) => status_code,
                Err(err) => {
                    tracing::error!("Invalid status code: {}", err);
                    return HttpResponse::build(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR)
                        .body(error_msg!("Invalid status code"));
                }
            };

            let mut builder = HttpResponse::build(actix_status_code);

            for header in headers.into_iter() {
                builder.append_header(header);
            }

            match body {
                ResponseBody::Stream(stream) => builder.streaming(stream),
                ResponseBody::Bytes(bytes) => builder.body(bytes),
                ResponseBody::Redirect(url) => builder.append_header(("Location", url)).body(""),
                ResponseBody::None => builder.body(""),
            }
        }
        None => HttpResponse::build(actix_web::http::StatusCode::INTERNAL_SERVER_ERROR)
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
        .request(to_reqwest_method(req.method()), forward_url)
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

    let mut client_resp = HttpResponseBuilder::new(to_actix_status_code(res.status()).unwrap());

    for (header_name, header_value) in res.headers().iter().filter(|(h, _)| *h != "connection") {
        client_resp.insert_header((header_name.as_str(), header_value.as_bytes()));
    }

    match res.bytes().await {
        Ok(bytes) => client_resp.body(bytes),
        Err(err) => {
            tracing::error!("Error reading response body from endpoint: {}", err);
            client_resp.body(error_msg!("Error reading response body from endpoint"))
        }
    }
}

fn to_actix_status_code(status_code: StatusCode) -> Result<actix_web::http::StatusCode, String> {
    actix_web::http::StatusCode::from_u16(status_code.as_u16())
        .map_err(|_| "Invalid status code".to_string())
}

// Actix uses http-0.2. However, the rest of the system uses
// http-1.x, so we need to convert between the two.
// Once Actix 5.x is released (which uses http-1.x), we can remove this mapping.
fn to_reqwest_method(method: &actix_web::http::Method) -> reqwest::Method {
    match *method {
        actix_web::http::Method::CONNECT => reqwest::Method::CONNECT,
        actix_web::http::Method::GET => reqwest::Method::GET,
        actix_web::http::Method::HEAD => reqwest::Method::HEAD,
        actix_web::http::Method::OPTIONS => reqwest::Method::OPTIONS,
        actix_web::http::Method::POST => reqwest::Method::POST,
        actix_web::http::Method::PUT => reqwest::Method::PUT,
        actix_web::http::Method::DELETE => reqwest::Method::DELETE,
        actix_web::http::Method::PATCH => reqwest::Method::PATCH,
        actix_web::http::Method::TRACE => reqwest::Method::TRACE,
        _ => {
            tracing::error!("Unsupported method: {}", method);
            panic!("Unsupported method: {}", method);
        }
    }
}
