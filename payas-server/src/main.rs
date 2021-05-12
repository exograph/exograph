use std::sync::Arc;

use actix_web::{web, App, HttpResponse, HttpServer, Responder};

use introspection::schema::Schema;
use payas_model::model::system::ModelSystem;
use serde_json::Value;

mod data;
mod execution;
mod introspection;

pub use payas_parser::ast;
pub use payas_parser::parser;
pub use payas_sql::sql;

mod test_util;

static PLAYGROUND_HTML: &str = include_str!("assets/playground.html");

async fn playground() -> impl Responder {
    HttpResponse::Ok().body(PLAYGROUND_HTML)
}

async fn resolve(
    req_body: String,
    schema: web::Data<Arc<(ModelSystem, Schema)>>,
) -> impl Responder {
    let (system, schema) = schema.as_ref().as_ref();

    let request: Value = serde_json::from_str(req_body.as_str()).unwrap();
    let operation_name = request["operationName"].as_str();
    let query_str = request["query"].as_str().unwrap();
    let variables = request["variables"].as_object();

    crate::execution::executor::execute(system, &schema, operation_name, query_str, variables)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let system = test_util::common_test_data::test_system();
    let schema = Schema::new(&system);

    let system_with_schema = Arc::new((system, schema));

    let server = HttpServer::new(move || {
        App::new()
            .data(system_with_schema.clone())
            .route("/", web::get().to(playground))
            .route("/", web::post().to(resolve))
    });

    server.bind("127.0.0.1:9876")?.run().await
}
