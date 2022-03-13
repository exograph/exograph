use async_stream::{try_stream, AsyncStream};
use execution::query_executor::QueryExecutor;
use introspection::schema::Schema;
use payas_deno::DenoExecutor;

use actix_web::web::Bytes;
use actix_web::{web, Error, HttpRequest, HttpResponse, Responder};
use anyhow::Result;
use payas_sql::asql::database_executor::DatabaseExecutor;

use crate::error::ExecutionError;
use crate::execution::query_context::QueryResponse;

use payas_model::{model::system::ModelSystem, sql::database::Database};
use serde_json::Value;

pub mod authentication;
mod data;
mod error;
pub mod execution;
mod introspection;

pub use payas_sql::sql;

use crate::authentication::{JwtAuthenticationError, JwtAuthenticator};

pub type SystemInfo = (ModelSystem, Schema, Database, DenoExecutor);

pub async fn resolve(
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
            let database_executor = DatabaseExecutor { database };
            let executor = QueryExecutor {
                system,
                schema,
                database_executor: &database_executor,
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

/// Creates the data required by the actix endpoint.
///
/// This should be added to the server as actix `app_data`.
pub fn create_system_info(system: ModelSystem, database: Database) -> SystemInfo {
    let schema = Schema::new(&system);
    let deno_executor = DenoExecutor::default();
    (system, schema, database, deno_executor)
}
