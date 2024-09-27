// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

mod request;

use std::path::Path;

use actix_web::{
    http::header::{CacheControl, CacheDirective},
    web::{self, Redirect, ServiceConfig},
    HttpRequest, HttpResponse, HttpResponseBuilder, Responder,
};
use url::Url;

use common::env_const::{get_deployment_mode, DeploymentMode};
use common::http::{RequestHead, RequestPayload, ResponsePayload};
use request::ActixRequestHead;
use resolver::{get_endpoint_http_path, get_playground_http_path, graphiql};
use router::SystemRouter;
use serde_json::Value;

macro_rules! error_msg {
    ($msg:literal) => {
        concat!("{\"errors\": [{\"message\":\"", $msg, "\"}]}").as_bytes()
    };
}

pub fn configure_router(system_router: web::Data<SystemRouter>) -> impl FnOnce(&mut ServiceConfig) {
    let resolve_path = get_endpoint_http_path();

    let endpoint_url = match get_deployment_mode() {
        Ok(DeploymentMode::Playground(url)) => Some(Url::parse(&url).unwrap()),
        _ => None,
    };

    move |app| {
        app.app_data(system_router)
            .app_data(web::Data::new(endpoint_url))
            .service(web::scope(&resolve_path).route("", web::to(resolve)));
    }
}

pub fn configure_playground(cfg: &mut ServiceConfig) {
    let playground_path = get_playground_http_path();
    let playground_path_subpaths = format!("{playground_path}/{{path:.*}}");

    async fn playground_redirect() -> impl Responder {
        Redirect::to(get_playground_http_path()).permanent()
    }

    // Serve GraphiQL playground from the playground path and all subpaths. Also set up a redirect
    // from the root path to the playground path (this way, users don't see an error ""No webpage
    // was found for the web address" when they go to the root path).
    cfg.route(&playground_path, web::get().to(playground))
        .route(&playground_path_subpaths, web::get().to(playground))
        .route("/", web::get().to(playground_redirect));
}

/// Resolve a GraphQL request
///
/// # Arguments
/// * `endpoint_url` - The target URL for resolving data (None implies that the current server is also the target)
async fn resolve(
    http_request: HttpRequest,
    body: web::Json<Value>,
    query: web::Query<Value>,
    endpoint_url: web::Data<Option<Url>>,
    system_router: web::Data<SystemRouter>,
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
    body: web::Json<Value>,
    query: Value,
    system_router: web::Data<SystemRouter>,
) -> HttpResponse {
    let playground_request = req
        .headers()
        .get("_exo_playground")
        .map(|value| value == "true")
        .unwrap_or(false);

    let request = ActixRequestPayload {
        head: ActixRequestHead::from_request(req, query),
        body: body.into_inner(),
    };

    let ResponsePayload {
        stream,
        headers,
        status_code,
    } = system_router
        .as_ref()
        .route(request, playground_request)
        .await;

    let mut builder = HttpResponse::build(status_code);

    for header in headers.into_iter() {
        builder.append_header(header);
    }

    match stream {
        Some(stream) => builder.streaming(stream),
        None => builder.finish(),
    }
}

async fn forward_request(
    req: HttpRequest,
    body: web::Json<Value>,
    forward_url: &Url,
) -> HttpResponse {
    let mut forward_url = forward_url.clone();
    forward_url.set_query(req.uri().query());

    let body = body.into_inner().to_string();

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

async fn playground(req: HttpRequest, resolver: web::Data<SystemRouter>) -> impl Responder {
    if !resolver.allow_introspection() {
        return HttpResponse::Forbidden().body("Introspection is not enabled");
    }

    let asset_path = req.match_info().get("path");

    // Adjust the path for "index.html" (which is requested with and empty path)
    let index = "index.html";
    let asset_path = asset_path
        .map(|path| if path.is_empty() { index } else { path })
        .unwrap_or(index);

    let asset_path = Path::new(asset_path);
    let extension = asset_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or(""); // If no extension, set it to an empty string, to use `actix_files::file_extension_to_mime`'s default behavior

    let content_type = actix_files::file_extension_to_mime(extension);

    // we shouldn't cache the index page, as we substitute in the endpoint path dynamically
    let cache_control = if index == "index.html" {
        CacheControl(vec![CacheDirective::NoCache])
    } else {
        CacheControl(vec![
            CacheDirective::Public,
            CacheDirective::MaxAge(60 * 60 * 24 * 365), // seconds in one year
        ])
    };

    match graphiql::get_asset_bytes(asset_path) {
        Some(asset) => HttpResponse::Ok()
            .content_type(content_type)
            .insert_header(cache_control)
            .body(asset),
        None => HttpResponse::NotFound().body("Not found"),
    }
}
