// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use actix_web::{App, HttpServer, middleware, web};

use common::env_const::get_mcp_http_path;
use server_actix::configure_router;
use thiserror::Error;
use tracing_actix_web::TracingLogger;

use std::net::SocketAddr;
use std::time;
use std::{io::ErrorKind, sync::Arc};

use common::{
    env_const::{
        DeploymentMode, EXO_ENABLE_MCP, EXO_SERVER_PORT, get_deployment_mode,
        get_graphql_http_path, get_playground_http_path,
    },
    introspection::{IntrospectionMode, introspection_mode},
};

use exo_env::{EnvError, Environment, SystemEnvironment};

const EXO_SERVER_HOST: &str = "EXO_SERVER_HOST";

#[derive(Error)]
enum ServerError {
    #[error("Port {0} is already in use. Check if there is another process running at that port.")]
    PortInUse(u16),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    EnvError(#[from] exo_env::EnvError),
    #[error("{0}")]
    ServerInitError(#[from] server_common::ServerInitError),
}

// A custom `Debug` implementation for `ServerError` (that delegate to the `Display` impl), so that
// we don't print the default `Debug` implementation's message when the server exits.
impl std::fmt::Debug for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

/// Run the server in production mode with a compiled exo_ir file
#[actix_web::main]
async fn main() -> Result<(), ServerError> {
    let start_time = time::SystemTime::now();

    let env = Arc::new(SystemEnvironment);

    let system_router = web::Data::new(server_common::init().await?);

    let server_port = env
        .get(EXO_SERVER_PORT)
        .map(|port_str| {
            port_str
                .parse::<u16>()
                .expect("Failed to parse EXO_SERVER_PORT")
        })
        .unwrap_or(9876);

    let env_clone = env.clone();

    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .wrap(middleware::NormalizePath::new(
                middleware::TrailingSlash::Trim,
            ))
            .configure(configure_router(system_router.clone(), env_clone.clone()))
    });

    let server_host = env.as_ref().get(EXO_SERVER_HOST);

    let server = match server_host {
        Some(host) => server.bind((host, server_port)),
        None => {
            match get_deployment_mode(env.as_ref())? {
                DeploymentMode::Dev | DeploymentMode::Yolo | DeploymentMode::Playground(_) => {
                    // Bind to "localhost" (needed for development). By binding to "localhost" we
                    // bind to both IPv4 and IPv6 loopback addresses ([::1]:9876, 127.0.0.1:9876)
                    //
                    // Note that tools such as "@graphql-codegen/cli" are unable to connect to
                    // "localhost:<port>" if we only bind to "0.0.0.0" or even "127.0.0.1" (but
                    // works fine, if we bind to IPv6 loopback address "::1").
                    server.bind(("localhost", server_port))
                }
                DeploymentMode::Prod => {
                    // Bind to "0.0.0.0" (all interfaces; needed for production; see the
                    // recommendation in `HttpServer::bind` documentation). This allows the server
                    // to be accessed from outside the host machine (e.g. when the server is in a
                    // Docker container in a fly.io deployment).
                    server.bind(("0.0.0.0", server_port))
                }
            }
        }
    };
    match server {
        Ok(server) => {
            let pretty_addr = pretty_addr(&server.addrs());

            let print_server_info = || {
                println!(
                    "Started server on {} in {:.2} ms",
                    pretty_addr,
                    start_time.elapsed().unwrap().as_micros() as f64 / 1000.0
                );
                println!("- GraphQL endpoint hosted at:");
                println!(
                    "\thttp://{pretty_addr}{}",
                    get_graphql_http_path(env.as_ref())
                );
                if env.as_ref().enabled(EXO_ENABLE_MCP, true)? {
                    println!("- MCP endpoint hosted at:");
                    println!("\thttp://{pretty_addr}{}", get_mcp_http_path(env.as_ref()));
                }

                Ok::<(), EnvError>(())
            };

            let print_playground_info = || {
                println!("- Playground hosted at:");
                println!(
                    "\thttp://{pretty_addr}{}",
                    get_playground_http_path(env.as_ref())
                );
            };

            match introspection_mode(&SystemEnvironment)? {
                IntrospectionMode::Enabled => {
                    print_server_info()?;
                    print_playground_info();
                }
                IntrospectionMode::Disabled => {
                    print_server_info()?;
                }
                IntrospectionMode::Only => {
                    print_playground_info();
                }
            }
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
        None => match addrs {
            // Print single address without square brackets
            [addr] => format!("{addr}"),
            _ => {
                format!("{addrs:?}")
            }
        },
    }
}
