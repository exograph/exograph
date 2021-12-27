use actix_web::dev::Server;
use async_stream::AsyncStream;
use bincode::deserialize_from;
use execution::executor::Executor;
use payas_deno::DenoExecutor;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::{env, sync::Arc};

use actix_cors::Cors;
use actix_web::rt::System;
use actix_web::web::Bytes;
use actix_web::Error;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use anyhow::{bail, Result};

use crate::error::ExecutionError;
use crate::execution::query_context::QueryResponse;

use async_stream::try_stream;

use introspection::schema::Schema;
use payas_model::{model::system::ModelSystem, sql::database::Database};
use serde_json::Value;

use std::time::{Duration, SystemTime};

mod authentication;
mod data;
mod error;
pub mod execution;
mod introspection;
pub mod model_watcher;
mod watcher;

pub use payas_sql::sql;

use crate::authentication::{JwtAuthenticationError, JwtAuthenticator};

static PLAYGROUND_HTML: &str = include_str!("assets/playground.html");

const SERVER_PORT_PARAM: &str = "CLAY_SERVER_PORT";

const FILE_WATCHER_DELAY: Duration = Duration::from_millis(200);

async fn playground() -> impl Responder {
    HttpResponse::Ok().body(PLAYGROUND_HTML)
}

pub type SystemInfo = Arc<(ModelSystem, Schema, Database, DenoExecutor)>;

async fn resolve(
    req: HttpRequest,
    body: web::Json<Value>,
    system_info: web::Data<SystemInfo>,
    authenticator: web::Data<Arc<JwtAuthenticator>>,
) -> impl Responder {
    let auth = authenticator.extract_authentication(req);

    // let to_bytes = Bytes::from;
    let to_bytes_static = |s: &'static str| Bytes::from_static(s.as_bytes());

    match auth {
        Ok(claims) => {
            let (system, schema, database, deno_execution) = system_info.as_ref().as_ref();
            let executor = Executor {
                system,
                schema,
                database,
                deno_execution,
            };
            let operation_name = body["operationName"].as_str();
            let query_str = body["query"].as_str().unwrap();
            let variables = body["variables"].as_object();

            match executor.execute(operation_name, query_str, variables, claims).await {
                Ok(parts) => {
                    let response_stream: AsyncStream<Result<Bytes, Error>, _> = try_stream! {
                        let parts_len = parts.len();

                        yield to_bytes_static(r#"{"data": {"#);
                        for (index, part) in parts.into_iter().enumerate() {
                            yield to_bytes_static("\"");
                            yield Bytes::from(part.0);
                            yield to_bytes_static(r#"":"#);
                            match part.1 {
                                QueryResponse::Json(value) => yield Bytes::from(value.to_string()),
                                QueryResponse::Raw(Some(value)) => yield Bytes::from(value),
                                QueryResponse::Raw(None) => yield to_bytes_static("null"),
                            };
                            if index != parts_len - 1 {
                                yield to_bytes_static(", ");
                            }
                        };
                        yield to_bytes_static("}}");
                    };

                    HttpResponse::Ok()
                        .content_type("application/json")
                        .streaming(Box::pin(response_stream))
                }
                Err(err) => {
                    let error_stream: AsyncStream<Result<Bytes, Error>, _> = try_stream! {
                        yield to_bytes_static(r#"{"errors": [{"message":""#);
                        yield Bytes::from(
                            // TODO: escape PostgreSQL errors properly here
                            format!("{}", err.chain().last().unwrap())
                                .replace("\"", "")
                                .replace("\n", "; ")
                        );
                        yield to_bytes_static(r#"""#);
                        eprintln!("{:?}", err);
                        if let Some(err) = err.downcast_ref::<ExecutionError>() {
                            yield to_bytes_static(r#", "locations": [{"line": "#);
                            yield Bytes::from(err.position().line.to_string());
                            yield to_bytes_static(r#", "column": "#);
                            yield Bytes::from(err.position().column.to_string());
                            yield to_bytes_static(r#"}]"#);
                        };
                        yield to_bytes_static(r#"}"#);
                        yield to_bytes_static("]}");
                    };

                    HttpResponse::Ok()
                        .content_type("application/json")
                        .streaming(Box::pin(error_stream))
                }
            }
        }
        Err(err) => {
            let (message, mut base_response) = match err {
                JwtAuthenticationError::ExpiredToken => {
                    ("Expired JWT token", HttpResponse::Unauthorized())
                }
                JwtAuthenticationError::TamperedToken => {
                    // No need to reveal more info for a tampered token, so mark is as a generic bad request
                    ("Unexpected error", HttpResponse::BadRequest())
                }
                JwtAuthenticationError::Unknown => ("Unknown error", HttpResponse::Unauthorized()),
            };

            let error_stream: AsyncStream<Result<Bytes, Error>, _> = try_stream! {
                yield to_bytes_static(r#"{"errors": [{"message":""#);
                yield to_bytes_static(message);
                yield to_bytes_static(r#""}]}"#);
            };

            base_response
                .content_type("application/json")
                .streaming(Box::pin(error_stream))
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

pub fn start_prod_mode(
    claypot_file: impl AsRef<Path> + Clone,
    system_start_time: Option<SystemTime>,
) -> Result<()> {
    if !Path::new(claypot_file.as_ref()).exists() {
        anyhow::bail!("File '{}' not found", claypot_file.as_ref().display());
    }

    let mut actix_system = System::new("claytip");

    match File::open(&claypot_file) {
        Ok(file) => {
            let claypot_file_buffer = BufReader::new(file);
            let in_file = BufReader::new(claypot_file_buffer);
            match deserialize_from(in_file) {
                Ok(system) => {
                    let server = start_server(system, system_start_time, false)?;
                    actix_system.block_on(server)?;
                }
                Err(e) => {
                    println!(
                        "Failed to read claypot file {:?}: {}",
                        claypot_file.as_ref(),
                        e
                    );
                    std::process::exit(1);
                }
            }

            Ok(())
        }
        Err(_) => {
            let message = format!("File {} doesn't exist. You need build it with the 'clay build <model-file-name>' command", claypot_file.as_ref().to_str().unwrap());
            println!("{}", message);
            Err(anyhow::anyhow!(message))
        }
    }
}

pub fn start_dev_mode(
    model_file: PathBuf,
    watch: bool,
    system_start_time: Option<SystemTime>,
) -> Result<()> {
    let mut actix_system = System::new("claytip");

    let model_file_clone = model_file.clone();
    let start_server = move |restart| {
        let system_start_time = if restart {
            Some(SystemTime::now())
        } else {
            system_start_time
        };
        let system = payas_parser::build_system(&model_file)?;

        start_server(system, system_start_time, restart)
    };

    if !watch {
        let server = start_server(false)?;
        actix_system.block_on(server)?;
        Ok(())
    } else {
        let stop_server = move |server: &mut Server| {
            actix_system.block_on(server.stop(true));
        };

        model_watcher::with_watch(
            &model_file_clone,
            FILE_WATCHER_DELAY,
            start_server,
            stop_server,
        )
    }
}

fn start_server(
    system: ModelSystem,
    system_start_time: Option<SystemTime>,
    restart: bool,
) -> Result<Server> {
    let database = Database::from_env(None)?; // TODO: error handling here
    let deno_executor = DenoExecutor::new();

    let schema = Schema::new(&system);
    let system_info = Arc::new((system, schema, database, deno_executor));
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

        match system_start_time {
            Some(system_start_time) => {
                let start_string = if restart { "Restarted" } else { "Started" };
                println!(
                    "{} server on {} in {}",
                    start_string,
                    addr,
                    duration_since_string(system_start_time.elapsed()?)
                )
            }
            None => println!("Started server on {}", addr),
        }
        Ok(server.run())
    } else {
        bail!("Error starting server on requested URL {}", server_url)
    }
}

fn duration_since_string(duration: Duration) -> String {
    let micros = duration.as_micros();

    format!("{:.2} milliseconds", (micros as f64 / 1000.0))
}
