use actix_web::{get, post, App, HttpResponse, HttpServer, Responder};

use crate::introspection::schema::Schema;
use serde_json::Value;

mod introspection;
mod model;

use crate::model::test_util::common_test_data::*;

static PLAYGROUND_HTML: &'static str = include_str!("assets/playground.html");

#[get("/")]
async fn playground() -> impl Responder {
    HttpResponse::Ok().body(PLAYGROUND_HTML)
}

#[post("/")]
async fn resolve(req_body: String) -> impl Responder {
    let v: Value = serde_json::from_str(req_body.as_str()).unwrap();

    let system = test_system();

    let example_schema = Schema::new(&system);

    let operation_name = v["operationName"].as_str().unwrap_or("");
    let query_str = v["query"].as_str().unwrap();
    let variables = v["variables"].as_object();

    let response = crate::introspection::executor::execute(
        &example_schema,
        operation_name,
        query_str,
        &variables,
    );

    HttpResponse::Ok().body(response)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().service(playground).service(resolve))
        .bind("127.0.0.1:9876")?
        .run()
        .await
}
