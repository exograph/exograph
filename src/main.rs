use actix_web::{dev::Body, get, post, App, HttpResponse, HttpServer, Responder};

use data::data_context::DataContext;
use introspection::schema::Schema;
use serde_json::Value;

mod introspection;
mod model;
#[macro_use]
mod sql;
mod data;
mod execution;

use crate::model::test_util::common_test_data::*;

static PLAYGROUND_HTML: &'static str = include_str!("assets/playground.html");

#[get("/")]
async fn playground() -> impl Responder {
    HttpResponse::Ok().body(PLAYGROUND_HTML)
}

#[post("/")]
async fn resolve(req_body: String) -> impl Responder {
    let request: Value = serde_json::from_str(req_body.as_str()).unwrap();

    let system = test_system();
    let database = test_database();
    let data_system = DataContext { system, database };

    let schema = Schema::new(&data_system.system); // TODO: Don't create schema every time

    let operation_name = request["operationName"].as_str().unwrap_or("");
    let query_str = request["query"].as_str().unwrap();
    let variables = request["variables"].as_object();

    let response = crate::execution::executor::execute(
        &data_system,
        &schema,
        operation_name,
        query_str,
        variables,
    );

    let response_bytes = response.as_bytes().to_owned();
    HttpResponse::Ok().body(Body::from(response_bytes))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().service(playground).service(resolve))
        .bind("127.0.0.1:9876")?
        .run()
        .await
}
