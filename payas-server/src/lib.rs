use actix_web::dev::Server;
use async_stream::AsyncStream;
use execution::executor::Executor;
use payas_deno::DenoExecutor;
use std::env;

use actix_cors::Cors;
use actix_web::web::{Bytes, Data};
use actix_web::Error;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use anyhow::Result;

use crate::error::ExecutionError;
use crate::execution::query_context::QueryResponse;

use async_stream::try_stream;

use introspection::schema::Schema;
use payas_model::{model::system::ModelSystem, sql::database::Database};
use serde_json::Value;

mod authentication;
mod data;
mod error;
pub mod execution;
mod introspection;

pub use payas_sql::sql;

use crate::authentication::{JwtAuthenticationError, JwtAuthenticator};

static PLAYGROUND_HTML: &str = include_str!("assets/playground.html");

const SERVER_PORT_PARAM: &str = "CLAY_SERVER_PORT";

async fn playground() -> impl Responder {
    HttpResponse::Ok().body(PLAYGROUND_HTML)
}

pub type SystemInfo = (ModelSystem, Schema, Database, DenoExecutor);

async fn resolve(
    req: HttpRequest,
    body: web::Json<Value>,
    system_info: web::Data<SystemInfo>,
    authenticator: web::Data<JwtAuthenticator>,
) -> impl Responder {
    let auth = authenticator.extract_authentication(req);

    // let to_bytes = Bytes::from;
    let to_bytes_static = |s: &'static str| Bytes::from_static(s.as_bytes());

    match auth {
        Ok(claims) => {
            let (system, schema, database, deno_execution) = system_info.as_ref();
            let executor = Executor {
                system,
                schema,
                database,
                deno_execution,
            };
            let operation_name = body["operationName"].as_str();
            let query_str = body["query"].as_str().unwrap();
            let variables = body["variables"].as_object();

            match executor
                .execute(operation_name, query_str, variables, claims)
                .await
            {
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

pub fn start_server(system: ModelSystem) -> Result<Server> {
    let database = Database::from_env(None)?; // TODO: error handling here
    let deno_executor = DenoExecutor::default();

    let schema = Schema::new(&system);
    let system_info = Data::new((system, schema, database, deno_executor));
    let authenticator = Data::new(JwtAuthenticator::new_from_env());

    let server = HttpServer::new(move || {
        let cors = cors_from_env();

        App::new()
            .wrap(cors)
            .app_data(system_info.clone())
            .app_data(authenticator.clone())
            .route("/", web::get().to(playground))
            .route("/", web::post().to(resolve))
    })
    .workers(1); // see payas-deno/executor.rs

    let server_port = env::var(SERVER_PORT_PARAM)
        .ok()
        .map(|port_str| port_str.parse::<u32>().unwrap())
        .unwrap_or(9876);

    let server_url = format!("0.0.0.0:{}", server_port);
    let server = server.bind(&server_url)?;

    println!("Started server on {}", server.addrs()[0]);

    Ok(server.run())
}
