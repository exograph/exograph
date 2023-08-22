// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpServer};

use resolver::{allow_introspection, get_endpoint_http_path, get_playground_http_path};
use server_actix::{configure_playground, configure_resolver};
use thiserror::Error;
use tracing_actix_web::TracingLogger;

use std::env;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::time;

const EXO_CORS_DOMAINS: &str = "EXO_CORS_DOMAINS";
const EXO_SERVER_PORT: &str = "EXO_SERVER_PORT";
const EXO_SERVER_HOST: &str = "EXO_SERVER_HOST";

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

    let server = HttpServer::new(move || {
        let cors = cors_from_env();

        App::new()
            .wrap(TracingLogger::default())
            .wrap(middleware::NormalizePath::new(
                middleware::TrailingSlash::Trim,
            ))
            .wrap(cors)
            .configure(configure_resolver(system_resolver.clone()))
            .configure(configure_playground)
    });

    // Bind to:
    // - "0.0.0.0" (all interfaces; needed for production; see the recommendation in `HttpServer::bind` documentation)
    // - "localhost" (needed for development). By binding to "localhost" we bind to both IPv4 and IPv6 loopback addresses
    //    ([::1]:9876, 127.0.0.1:9876)
    //
    // Note that tools such as "@graphql-codegen/cli" are unable to connect to "localhost:<port>" if we
    // only bind to "0.0.0.0" or even "127.0.0.1".
    // let server = server
    //     .bind(("0.0.0.0", server_port)) // bind to all interfaces (needed for production)
    //     .and_then(|server| server.bind(("localhost", server_port))); // bind to localhost (needed for development; for example, )
    let server_host = env::var(EXO_SERVER_HOST);

    let server = match server_host {
        Ok(host) => server.bind((host, server_port)),
        Err(_) => {
            server
                .bind(("0.0.0.0", server_port)) // bind to all interfaces (needed for production)
                .and_then(|server| server.bind(("localhost", server_port))) // bind to localhost (needed for development; for example, )
        }
    };
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
