use std::{env, sync::Arc};

use actix_cors::Cors;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};

use introspection::schema::Schema;
use payas_model::{model::system::ModelSystem, sql::database::Database};
use payas_parser::builder::system_builder;
use serde_json::Value;

mod data;
mod execution;
mod introspection;

pub use payas_parser::ast;
pub use payas_parser::parser;
pub use payas_sql::sql;

static PLAYGROUND_HTML: &str = include_str!("assets/playground.html");

const SERVER_PORT_PARAM: &str = "PAYAS_SERVER_PORT";

async fn playground() -> impl Responder {
    HttpResponse::Ok().body(PLAYGROUND_HTML)
}

async fn resolve(
    req_body: String,
    system_info: web::Data<Arc<(ModelSystem, Schema, Database)>>,
) -> impl Responder {
    let (system, schema, database) = system_info.as_ref().as_ref();

    let request: Value = serde_json::from_str(req_body.as_str()).unwrap();
    let operation_name = request["operationName"].as_str();
    let query_str = request["query"].as_str().unwrap();
    let variables = request["variables"].as_object();

    crate::execution::executor::execute(
        system,
        schema,
        database,
        operation_name,
        query_str,
        variables,
    )
}

fn cors_from_env() -> Cors {
    const CORS_DOMAINS_PARAM: &str = "PAYAS_CORS_DOMAINS";

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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let ast_system = parser::parse_file(&args[1]);
    let system = system_builder::build(ast_system);
    let schema = Schema::new(&system);

    let database = Database::from_env();
    database.create_client(); // Fail on startup if the database is misconfigured (TODO: provide an option to not do so)

    let system_info = Arc::new((system, schema, database));

    let server = HttpServer::new(move || {
        let cors = cors_from_env();

        App::new()
            .wrap(cors)
            .data(system_info.clone())
            .route("/", web::get().to(playground))
            .route("/", web::post().to(resolve))
    });

    let server_port = env::var(SERVER_PORT_PARAM)
        .ok()
        .map(|port_str| port_str.parse::<u32>().unwrap())
        .unwrap_or(9876);

    let server_url = format!("0.0.0.0:{}", server_port);

    println!("Started server on {}", server_url);
    server.bind(&server_url)?.run().await
}
