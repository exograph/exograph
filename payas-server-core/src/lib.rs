/// Provides core functionality for handling incoming queries without depending
/// on any specific web framework.
///
/// The `resolve` function is responsible for doing the work, using information
/// extracted from an incoing request, and returning the response as a stream.
use anyhow::Result;
use async_stream::try_stream;
use bytes::Bytes;
use error::ExecutionError;
use execution::query_context::QueryResponse;
use execution::query_executor::QueryExecutor;
use futures::Stream;
use introspection::schema::Schema;
use payas_deno::DenoExecutor;
use payas_model::model::system::ModelSystem;
use payas_sql::{Database, DatabaseExecutor};
use request_context::RequestContext;
use serde_json::{Map, Value};

mod data;
mod error;
mod execution;
mod introspection;
pub mod request_context;

/// Opaque type encapsulating the information required by the `resolve`
/// function.
///
/// A server implmentation should call `create_system_info` and store the
/// returned value, passing a reference to it each time it calls `resolve`.
///
/// For example, in actix, this should be added to the server using `app_data`.
pub struct SystemInfo(ModelSystem, Schema, Database, DenoExecutor);

/// Creates the data required by the server endpoint.
///
/// It's assumed that the server has already loaded a claypot file to obtain
/// a `ModelSystem` instance. It also provides the `Database` instance which
/// it wishes to use for storage.
pub fn create_system_info(system: ModelSystem, database: Database) -> SystemInfo {
    let schema = Schema::new(&system);
    let deno_executor = DenoExecutor::default();
    SystemInfo(system, schema, database, deno_executor)
}

/// Resolves an incoming query, returning a response stream which containing JSON
/// which can either be the data returned by the query, or a list of errors if
/// something went wrong.
pub async fn resolve<E>(
    system_info: &SystemInfo,
    operation_name: Option<&str>,
    query_str: &str,
    variables: Option<&Map<String, Value>>,
    request_context: RequestContext,
) -> impl Stream<Item = Result<Bytes, E>> {
    let SystemInfo(system, schema, database, deno_execution) = system_info;

    let database_executor = DatabaseExecutor { database };
    let executor = QueryExecutor {
        system,
        schema,
        database_executor: &database_executor,
        deno_execution,
    };

    let response = executor
        .execute(operation_name, query_str, variables, request_context)
        .await;

    try_stream! {
        match response {
            Ok(parts) => {
                let parts_len = parts.len();
                yield Bytes::from_static(br#"{"data": {"#);
                for (index, part) in parts.into_iter().enumerate() {
                    yield Bytes::from_static(b"\"");
                    yield Bytes::from(part.0);
                    yield Bytes::from_static(br#"":"#);
                    match part.1 {
                        QueryResponse::Json(value) => yield Bytes::from(value.to_string()),
                        QueryResponse::Raw(Some(value)) => yield Bytes::from(value),
                        QueryResponse::Raw(None) => yield Bytes::from_static(b"null"),
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
                    format!("{}", err.chain().last().unwrap())
                        .replace("\"", "")
                        .replace("\n", "; ")
                );
                yield Bytes::from_static(br#"""#);
                eprintln!("{:?}", err);
                if let Some(err) = err.downcast_ref::<ExecutionError>() {
                    yield Bytes::from_static(br#", "locations": [{"line": "#);
                    yield Bytes::from(err.position().line.to_string());
                    yield Bytes::from_static(br#", "column": "#);
                    yield Bytes::from(err.position().column.to_string());
                    yield Bytes::from_static(br#"}]"#);
                };
                yield Bytes::from_static(br#"}"#);
                yield Bytes::from_static(b"]}");
            },
        }
    }
}
