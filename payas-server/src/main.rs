use std::{env, sync::Arc};

use actix_cors::Cors;
use actix_web::http::StatusCode;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use anyhow::{anyhow, bail, Result};

use introspection::schema::Schema;
use payas_model::{model::system::ModelSystem, sql::database::Database};
use payas_parser::builder::system_builder;
use serde_json::Value;

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

            crate::execution::executor::execute(
                system,
                schema,
                database,
                operation_name,
                query_str,
                variables,
                claims,
            )
            .with_status(StatusCode::OK)
            .with_header("Content-Type", "application/json")
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

// TODO: Avoid duplication from cli's main.rs
const DEFAULT_MODEL_FILE: &str = "index.clay";

#[actix_web::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let model_file = args
        .get(1)
        .map(|arg| arg.as_str())
        .unwrap_or(DEFAULT_MODEL_FILE);
    let (ast_system, codemap) = parser::parse_file(model_file);
    let system = system_builder::build(ast_system, codemap);
    let schema = Schema::new(&system);

    let database = Database::from_env().unwrap(); // TODO: error handling here
    database.create_client().unwrap(); // Fail on startup if the database is misconfigured (TODO: provide an option to not do so)

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
        server.run().await.map_err(|e| anyhow!(e))
    } else {
        bail!("Error starting server on requested URL {}", server_url);
    }
}
