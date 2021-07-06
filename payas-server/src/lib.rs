use std::path::Path;
use std::thread;
use std::{env, sync::Arc};

use actix_cors::Cors;
use actix_web::http::StatusCode;
use actix_web::rt::System;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use anyhow::{bail, Result};

use introspection::schema::Schema;
use payas_model::{model::system::ModelSystem, sql::database::Database};
use payas_parser::builder::system_builder;
use serde_json::Value;

use notify::{self, DebouncedEvent, RecursiveMode, Watcher};
use std::sync::mpsc;
use std::time::Duration;

mod authentication;
mod data;
mod execution;
mod introspection;

pub use payas_parser::ast;
pub use payas_parser::parser;
pub use payas_sql::sql;

use crate::authentication::{JwtAuthenticationError, JwtAuthenticator};

static PLAYGROUND_HTML: &str = include_str!("assets/playground.html");

const SERVER_PORT_PARAM: &str = "CLAY_SERVER_PORT";

const FILE_WATCHER_DELAY: Duration = Duration::from_millis(200);

async fn playground() -> impl Responder {
    HttpResponse::Ok().body(PLAYGROUND_HTML)
}

async fn resolve(
    req: HttpRequest,
    body: web::Json<Value>,
    system_info: web::Data<Arc<(ModelSystem, Schema, Database)>>,
    authenticator: web::Data<Arc<JwtAuthenticator>>,
) -> impl Responder {
    let auth = authenticator.extract_authentication(req);

    match auth {
        Ok(claims) => {
            let (system, schema, database) = system_info.as_ref().as_ref();

            let operation_name = body["operationName"].as_str();
            let query_str = body["query"].as_str().unwrap();
            let variables = body["variables"].as_object();

            match crate::execution::executor::execute(
                system,
                schema,
                database,
                operation_name,
                query_str,
                variables,
                claims,
            ) {
                Ok(response) => response
                    .with_status(StatusCode::OK)
                    .with_header("Content-Type", "application/json"),
                Err(err) => {
                    let mut response = String::from(r#"{"errors": [{"message":""#);
                    response.push_str(&format!("{}", err));
                    response.push_str(r#""}]}"#);

                    response
                        .with_status(StatusCode::OK)
                        .with_header("Content-Type", "application/json")
                }
            }
        }
        Err(err) => {
            let (message, status_code) = match err {
                JwtAuthenticationError::ExpiredToken => {
                    ("Expired JWT token", StatusCode::UNAUTHORIZED)
                }
                JwtAuthenticationError::TamperedToken => {
                    // No need to reveal more info for a tampered token, so mark is as a generic bad request
                    ("Unexpected error", StatusCode::BAD_REQUEST)
                }
                JwtAuthenticationError::Unknown => ("Unknown error", StatusCode::UNAUTHORIZED),
            };

            let mut response = String::from(r#"{"errors": [{"message":""#);
            response.push_str(message);
            response.push_str(r#""}]}"#);

            response
                .with_status(status_code)
                .with_header("Content-Type", "application/json")
        }
    }
}

fn cors_from_env() -> Cors {
    const CORS_DOMAINS_PARAM: &str = "CLAY_CORS_DOMAINS";

    match env::var(CORS_DOMAINS_PARAM).ok() {
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

enum ServerLoopEvent {
    FileChange,
    SigInt,
}

pub fn main(model_file: impl AsRef<Path>, watch: bool) -> Result<()> {
    let (tx, rx) = mpsc::channel();

    // Watch for claytip model file edits
    if watch {
        let model_file = model_file.as_ref().to_path_buf();
        let tx = tx.clone();
        thread::spawn(move || -> Result<()> {
            let (watcher_tx, watcher_rx) = mpsc::channel();
            let mut watcher = notify::watcher(watcher_tx, FILE_WATCHER_DELAY)?;
            watcher.watch(&model_file, RecursiveMode::NonRecursive)?;

            loop {
                match watcher_rx.recv() {
                    Ok(e) => {
                        if matches!(e, DebouncedEvent::Write(_)) {
                            tx.send(ServerLoopEvent::FileChange)?;
                        }
                    }
                    Err(e) => bail!(e),
                }
            }
        });
    }

    // Watch for ctrl-c (SIGINT)
    ctrlc::set_handler(move || {
        tx.send(ServerLoopEvent::SigInt).unwrap();
    })?;

    let mut actix_system = System::new("claytip");

    loop {
        let model_file = model_file.as_ref().to_path_buf();

        let (ast_system, codemap) = parser::parse_file(model_file);
        let system = system_builder::build(ast_system, codemap);
        let schema = Schema::new(&system);

        let database = Database::from_env()?; // TODO: error handling here
        database.create_client()?; // Fail on startup if the database is misconfigured (TODO: provide an option to not do so)

        let system_info = Arc::new((system, schema, database));
        let authenticator = Arc::new(JwtAuthenticator::new_from_env());

        let server = HttpServer::new(move || {
            let cors = cors_from_env();

            App::new()
                .wrap(cors)
                .data(system_info.clone())
                .data(authenticator.clone())
                .route("/", web::get().to(playground))
                .route("/", web::post().to(resolve))
        });

        let server_port = env::var(SERVER_PORT_PARAM)
            .ok()
            .map(|port_str| port_str.parse::<u32>().unwrap())
            .unwrap_or(9876);

        let server_url = format!("0.0.0.0:{}", server_port);
        let result = server.bind(&server_url);

        if let Ok(server) = result {
            let addr = server.addrs()[0];

            println!("Started server on {}", addr);
            let server = server.run();

            // Stop and restart the server initializtion loop when the model file is edited. Exit
            // the server loop when SIGINT is received.
            match rx.recv()? {
                ServerLoopEvent::FileChange => {
                    println!("Restarting...");
                    actix_system.block_on(async move {
                        server.stop(true).await;
                    });
                }
                ServerLoopEvent::SigInt => {
                    println!("Exiting");
                    break;
                }
            }
        } else {
            bail!("Error starting server on requested URL {}", server_url)
        }
    }

    Ok(())
}
