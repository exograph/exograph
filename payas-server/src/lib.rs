use actix_web::dev::Server;
use async_stream::AsyncStream;
use std::path::Path;
use std::{env, sync::Arc};

use actix_cors::Cors;
use actix_web::rt::System;
use actix_web::web::Bytes;
use actix_web::Error;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use anyhow::{bail, Result};

use crate::execution::query_context::QueryResponse;

use async_stream::try_stream;

use introspection::schema::Schema;
use payas_model::{model::system::ModelSystem, sql::database::Database};
use payas_parser::builder;
use serde_json::Value;

use std::time::Duration;

mod authentication;
mod data;
mod execution;
mod introspection;
mod watcher;

pub use payas_parser::ast;
pub use payas_parser::parser;
pub use payas_sql::sql;

use crate::authentication::{JwtAuthenticationError, JwtAuthenticator};

static PLAYGROUND_HTML: &str = include_str!("assets/playground.html");

const SERVER_PORT_PARAM: &str = "CLAY_SERVER_PORT";
const CONNECTION_POOL_SIZE_PARAM: &str = "CLAY_CONNECTION_POOL_SIZE";
const CHECK_CONNECTION_ON_STARTUP: &str = "CLAY_CHECK_CONNECTION_ON_STARTUP";

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

    let to_bytes = |s: String| Bytes::from(s);
    let to_bytes_static = |s: &'static str| Bytes::from_static(s.as_bytes());

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
                Ok(parts) => {
                    let response_stream: AsyncStream<Result<Bytes, Error>, _> = try_stream! {
                        let parts_len = parts.len();

                        yield to_bytes_static(r#"{"data": {"#);
                        for (index, part) in parts.into_iter().enumerate() {
                            yield to_bytes_static("\"");
                            yield to_bytes(part.0);
                            yield to_bytes_static(r#"":"#);
                            match part.1 {
                                QueryResponse::Json(value) => yield to_bytes(value.to_string()),
                                QueryResponse::Raw(Some(value)) => yield to_bytes(value),
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
                        yield to_bytes(format!("{}", err.chain().last().unwrap()));
                        eprintln!("{:?}", err);
                        yield to_bytes_static(r#""}]}"#);
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

pub fn start_dev_mode(model_file: impl AsRef<Path> + Clone, watch: bool) -> Result<()> {
    let mut actix_system = System::new("claytip");

    let model_file_clone = model_file.clone();
    let start_server = move || {
        let (ast_system, codemap) = parser::parse_file(&model_file);
        let system = builder::build(ast_system, codemap)?;
        let schema = Schema::new(&system);

        start_server(system, schema)
    };

    if watch {
        start_server().map(|_| ())
    } else {
        let stop_server = move |server: &mut Server| {
            actix_system.block_on(server.stop(true));
        };

        watcher::with_watch(
            &model_file_clone,
            FILE_WATCHER_DELAY,
            start_server,
            stop_server,
        )
    }
}

fn create_database() -> Result<Database> {
    let pool_size = env::var(CONNECTION_POOL_SIZE_PARAM)
        .ok()
        .map(|pool_str| pool_str.parse::<u32>().unwrap())
        .unwrap_or(10);

    let db = Database::from_env(pool_size);

    match db {
        Ok(db) => {
            let check_connection = env::var(CHECK_CONNECTION_ON_STARTUP)
                .ok()
                .map(|pool_str| pool_str.parse::<bool>().unwrap())
                .unwrap_or(true);
            if check_connection {
                db.get_client()?; // Fail on startup if the database is misconfigured
            }
            Ok(db)
        }
        e @ Err(_) => e,
    }
}

fn start_server(system: ModelSystem, schema: Schema) -> Result<Server> {
    let database = create_database()?; // TODO: error handling here

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
        Ok(server.run())
    } else {
        bail!("Error starting server on requested URL {}", server_url)
    }
}
