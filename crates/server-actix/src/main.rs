// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use actix_cors::Cors;
use actix_web::http::header::{CacheControl, CacheDirective};
use actix_web::{middleware, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use core_resolver::system_resolver::SystemResolver;

use resolver::{allow_introspection, get_endpoint_http_path, get_playground_http_path, graphiql};
use server_actix::resolve;
use thiserror::Error;
use tracing_actix_web::TracingLogger;

use std::env;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::path::Path;
use std::time;

const EXO_CORS_DOMAINS: &str = "EXO_CORS_DOMAINS";
const EXO_SERVER_PORT: &str = "EXO_SERVER_PORT";

#[derive(Error)]
enum ServerError {
    #[error("Port {0} is already in use. Check if there is another process running at that port.")]
    PortInUse(u16),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

// A custom `Debug` implementation for `ServerError` (that delegate to the `Display` impl), so that
// we don't print the default `Debug` implementation's message when the server exits.
impl std::fmt::Debug for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

/// Run the server in production mode with a compiled exo_ir file
#[actix_web::main]
async fn main() -> Result<(), ServerError> {
    let start_time = time::SystemTime::now();

    let system_resolver = web::Data::new(server_common::init().await);

    let server_port = env::var(EXO_SERVER_PORT)
        .map(|port_str| {
            port_str
                .parse::<u16>()
                .expect("Failed to parse EXO_SERVER_PORT")
        })
        .unwrap_or(9876);

    let resolve_path = get_endpoint_http_path();
    let playground_path = get_playground_http_path();
    let playground_path_subpaths = format!("{playground_path}/{{path:.*}}");

    let server = HttpServer::new(move || {
        let cors = cors_from_env();

        App::new()
            .wrap(TracingLogger::default())
            .wrap(middleware::NormalizePath::new(
                middleware::TrailingSlash::Trim,
            ))
            .wrap(cors)
            .app_data(system_resolver.clone())
            .route(&resolve_path, web::post().to(resolve))
            .route(&playground_path, web::get().to(playground))
            .route(&playground_path_subpaths, web::get().to(playground))
    });

    // Bind to:
    // - "0.0.0.0" (all interfaces; needed for production; see the recommendation in `HttpServer::bind` documentation)
    // - "localhost" (needed for development). By binding to "localhost" we bind to both IPv4 and IPv6 loopback addresses
    //    ([::1]:9876, 127.0.0.1:9876)
    //
    // Note that tools such as "@graphql-codegen/cli" are unable to connect to "localhost:<port>" if we
    // only bind to "0.0.0.0" or even "127.0.0.1".
    let server = server
        .bind(("0.0.0.0", server_port)) // bind to all interfaces (needed for production)
        .and_then(|server| server.bind(("localhost", server_port))); // bind to localhost (needed for development; for example, )

    match server {
        Ok(server) => {
            let pretty_addr = pretty_addr(&server.addrs());
            println!(
                "Started server on {} in {:.2} ms",
                pretty_addr,
                start_time.elapsed().unwrap().as_micros() as f64 / 1000.0
            );

            if let Ok(true) = allow_introspection() {
                println!("- Playground hosted at:");
                println!("\thttp://{pretty_addr}{}", get_playground_http_path());
            }

            println!("- Endpoint hosted at:");
            println!("\thttp://{pretty_addr}{}", get_endpoint_http_path());

            Ok(server.run().await?)
        }
        Err(e) => Err(if e.kind() == ErrorKind::AddrInUse {
            ServerError::PortInUse(server_port)
        } else {
            ServerError::Io(e)
        }),
    }
}

fn pretty_addr(addrs: &[SocketAddr]) -> String {
    let loopback_addr = addrs.iter().find(|addr| addr.ip().is_loopback());

    match loopback_addr {
        Some(addr) => format!("localhost:{}", addr.port()),
        None => format!("{:?}", addrs),
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

fn cors_from_env() -> Cors {
    match env::var(EXO_CORS_DOMAINS).ok() {
        Some(domains) => {
            let domains_list = domains.split(',');

            let cors = domains_list.fold(Cors::default(), |cors, domain| {
                if domain == "*" {
                    cors.allow_any_origin()
                } else {
                    cors.allowed_origin(domain)
                }
            });

            // TODO: Allow more control over headers, max_age etc
            cors.allowed_methods(vec!["GET", "POST"])
                .allow_any_header()
                .max_age(3600)
        }
        None => Cors::default(),
    }
}
