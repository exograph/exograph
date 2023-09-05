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
    web::{self, Bytes, Redirect, ServiceConfig},
    Error, HttpRequest, HttpResponse, Responder,
};

use core_resolver::context::{ContextExtractionError, RequestContext};
use core_resolver::system_resolver::SystemResolver;
use core_resolver::OperationsPayload;
use request::ActixRequest;
use resolver::{get_endpoint_http_path, get_playground_http_path, graphiql};
use serde_json::Value;

macro_rules! error_msg {
    ($msg:literal) => {
        concat!("{\"errors\": [{\"message\":\"", $msg, "\"}]}").as_bytes()
    };
}

pub fn configure_resolver(
    system_resolver: web::Data<SystemResolver>,
) -> impl FnOnce(&mut ServiceConfig) {
    let resolve_path = get_endpoint_http_path();

    move |app| {
        app.app_data(system_resolver)
            .service(web::scope(&resolve_path).route("", web::post().to(resolve)));
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

async fn resolve(
    req: HttpRequest,
    body: web::Json<Value>,
    system_resolver: web::Data<SystemResolver>,
) -> impl Responder {
    let request = ActixRequest::from_request(req);
    let request_context = RequestContext::new(&request, vec![], system_resolver.as_ref());

    match request_context {
        Ok(request_context) => {
            let operations_payload: Result<OperationsPayload, _> =
                serde_json::from_value(body.into_inner());

            match operations_payload {
                Ok(operations_payload) => {
                    let (stream, headers) = resolver::resolve::<Error>(
                        operations_payload,
                        system_resolver.as_ref(),
                        request_context,
                    )
                    .await;

                    let mut builder = HttpResponse::Ok();
                    builder.content_type("application/json");

                    for header in headers.into_iter() {
                        builder.append_header(header);
                    }

                    builder.streaming(Box::pin(stream))
                }
                Err(_) => HttpResponse::BadRequest().body(error_msg!("Invalid query payload")),
            }
        }

        Err(err) => {
            let (message, mut base_response) = match err {
                ContextExtractionError::Unauthorized => {
                    (error_msg!("Unauthorized"), HttpResponse::Unauthorized())
                }
                ContextExtractionError::Malformed => {
                    (error_msg!("Malformed header"), HttpResponse::BadRequest())
                }
                _ => (error_msg!("Unknown error"), HttpResponse::Unauthorized()),
            };

            let error_message: Result<Bytes, Error> = Ok(Bytes::from_static(message));

            base_response
                .content_type("application/json")
                .streaming(Box::pin(futures::stream::once(async { error_message })))
        }
    }
}

async fn playground(req: HttpRequest, resolver: web::Data<SystemResolver>) -> impl Responder {
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
