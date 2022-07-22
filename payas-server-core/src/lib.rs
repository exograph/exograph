use std::{fs::File, io::BufReader, path::Path, pin::Pin};

/// Provides core functionality for handling incoming queries without depending
/// on any specific web framework.
///
/// The `resolve` function is responsible for doing the work, using information
/// extracted from an incoming request, and returning the response as a stream.
use ::tracing::{error, instrument};
use async_graphql_parser::Pos;
use async_stream::try_stream;
use bincode::deserialize_from;
use bytes::Bytes;
pub use execution::system_context::SystemContext;
use futures::Stream;
use initialization_error::InitializationError;
use introspection::schema::Schema;
use payas_deno::DenoExecutorPool;
use payas_model::model::system::ModelSystem;
use payas_sql::{Database, DatabaseExecutor};
use request_context::RequestContext;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::{execution::query_response::QueryResponseBody, execution_error::ExecutionError};

mod data;
mod deno;
mod deno_integration;
mod execution;
mod execution_error;
pub mod graphiql;
pub mod initialization_error;
mod introspection;
mod logging_tracing;
pub mod request_context;
mod resolver;
mod validation;
mod validation_error;

fn open_claypot_file(claypot_file: &str) -> Result<ModelSystem, InitializationError> {
    if !Path::new(&claypot_file).exists() {
        return Err(InitializationError::FileNotFound(claypot_file.to_string()));
    }
    match File::open(&claypot_file) {
        Ok(file) => {
            let claypot_file_buffer = BufReader::new(file);
            let in_file = BufReader::new(claypot_file_buffer);
            deserialize_from(in_file).map_err(|err| {
                InitializationError::ClaypotDeserialization(claypot_file.into(), err)
            })
        }
        Err(e) => Err(InitializationError::FileOpen(claypot_file.into(), e)),
    }
}

pub fn create_system_context(claypot_file: &str) -> Result<SystemContext, InitializationError> {
    let database = Database::from_env(None).expect("Failed to access database"); // TODO: error handling here

    let allow_introspection = match std::env::var("CLAY_INTROSPECTION").ok() {
        Some(e) => match e.parse::<bool>() {
            Ok(e) => e,
            Err(_) => {
                error!("CLAY_INTROSPECTION env var must be set to either true or false");
                std::process::exit(1);
            }
        },
        None => false,
    };

    let system = open_claypot_file(claypot_file)?;
    let schema = Schema::new(&system);
    let deno_execution_config = DenoExecutorPool::new_from_config(deno_integration::clay_config());

    let database_executor = DatabaseExecutor { database };

    let executor = SystemContext {
        database_executor,
        deno_execution_pool: deno_execution_config,
        system,
        schema,
        allow_introspection,
    };

    Ok(executor)
}

#[derive(Debug, Deserialize)]
pub struct OperationsPayload {
    #[serde(rename = "operationName")]
    operation_name: Option<String>,
    query: String,
    variables: Option<Map<String, Value>>,
}

pub type Headers = Vec<(String, String)>;

/// Initializes logging for payas-server-core.
pub fn init() {
    logging_tracing::init()
}

/// Resolves an incoming query, returning a response stream containing JSON and a set
/// of HTTP headers. The JSON may be either the data returned by the query, or a list of errors
/// if something went wrong.
///
/// In a typical use case (for example payas-server-actix), the caller will
/// first call `create_system_context` to create an `OperationsExecutor` object, and
/// then call `resolve` with that object.
#[instrument(
    name = "payas-server-core::resolve"
    skip(system_context, request_context)
    )]
pub async fn resolve<'a, E: 'static>(
    operations_payload: OperationsPayload,
    system_context: &SystemContext,
    request_context: RequestContext<'a>,
) -> (Pin<Box<dyn Stream<Item = Result<Bytes, E>>>>, Headers) {
    let response = system_context
        .execute(operations_payload, &request_context)
        .await;

    let headers = if let Ok(ref response) = response {
        response
            .iter()
            .flat_map(|(_, qr)| qr.headers.clone())
            .collect()
    } else {
        vec![]
    };

    let stream = try_stream! {
        macro_rules! report_position {
            ($position:expr) => {
                let p: Pos = $position;

                yield Bytes::from_static(br#"{"line": "#);
                yield Bytes::from(p.line.to_string());
                yield Bytes::from_static(br#", "column": "#);
                yield Bytes::from(p.column.to_string());
                yield Bytes::from_static(br#"}"#);
            };
        }

        match response {
            Ok(parts) => {
                let parts_len = parts.len();
                yield Bytes::from_static(br#"{"data": {"#);
                for (index, part) in parts.into_iter().enumerate() {
                    yield Bytes::from_static(b"\"");
                    yield Bytes::from(part.0);
                    yield Bytes::from_static(br#"":"#);
                    match part.1.body {
                        QueryResponseBody::Json(value) => yield Bytes::from(value.to_string()),
                        QueryResponseBody::Raw(Some(value)) => yield Bytes::from(value),
                        QueryResponseBody::Raw(None) => yield Bytes::from_static(b"null"),
                    };
                    if index != parts_len - 1 {
                        yield Bytes::from_static(b", ");
                    }
                };
                yield Bytes::from_static(b"}}");
            },
            Err(err) => {
                yield Bytes::from_static(br#"{"errors": [{"message":""#);
                yield Bytes::from(
                    // TODO: escape PostgreSQL errors properly here
                    format!("{}", err.user_error_message())
                        .replace("\"", "")
                        .replace("\n", "; ")
                );
                yield Bytes::from_static(br#"""#);
                if let ExecutionError::Validation(err) = err {
                    yield Bytes::from_static(br#", "locations": ["#);
                    report_position!(err.position1());
                    if let Some(position2) = err.position2() {
                        yield Bytes::from_static(br#","#);
                        report_position!(position2);
                    }
                    yield Bytes::from_static(br#"]"#);
                };
                yield Bytes::from_static(br#"}"#);
                yield Bytes::from_static(b"]}");
            },
        }
    };

    let boxed_stream = Box::pin(stream) as Pin<Box<dyn Stream<Item = Result<Bytes, E>>>>;

    (boxed_stream, headers)
}

pub fn get_playground_http_path() -> String {
    std::env::var("CLAY_PLAYGROUND_HTTP_PATH").unwrap_or_else(|_| "/playground".to_string())
}

pub fn get_endpoint_http_path() -> String {
    std::env::var("CLAY_ENDPOINT_HTTP_PATH").unwrap_or_else(|_| "/graphql".to_string())
}
