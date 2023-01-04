use std::pin::Pin;
use std::process::exit;
use std::{fs::File, io::BufReader, path::Path};

use crate::system_loader::SystemLoadingError;

use super::system_loader::SystemLoader;
use ::tracing::instrument;
use async_graphql_parser::Pos;
use async_stream::try_stream;
use bytes::Bytes;
use core_resolver::system_resolver::SystemResolutionError;
use core_resolver::system_resolver::SystemResolver;
pub use core_resolver::OperationsPayload;
use core_resolver::{request_context::RequestContext, QueryResponseBody};
use futures::Stream;

pub type Headers = Vec<(String, String)>;

/// Resolves an incoming query, returning a response stream containing JSON and a set
/// of HTTP headers. The JSON may be either the data returned by the query, or a list of errors
/// if something went wrong.
///
/// In a typical use case (for example server-actix), the caller will
/// first call `create_system_resolver_or_exit` to create a [SystemResolver] object, and
/// then call `resolve` with that object.
#[instrument(
    name = "resolver::resolve"
    skip(system_resolver, request_context)
    )]
pub async fn resolve<'a, E: 'static>(
    operations_payload: OperationsPayload,
    system_resolver: &SystemResolver,
    request_context: RequestContext<'a>,
) -> (Pin<Box<dyn Stream<Item = Result<Bytes, E>>>>, Headers) {
    let response = system_resolver
        .resolve_operations(operations_payload, &request_context)
        .await;

    let headers = if let Ok(ref response) = response {
        response
            .iter()
            .flat_map(|(_, qr)| qr.headers.clone())
            .collect()
    } else {
        vec![]
    };

    let ctx = request_context.get_base_context();
    let mut tx_holder = ctx.transaction_holder.try_lock().unwrap();

    let response = tx_holder
        .finalize(response.is_ok())
        .await
        .map_err(|e| {
            SystemResolutionError::Generic(format!("Error while finalizing transaction: {}", e))
        })
        .and(response);

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

        macro_rules! report_positions {
            ($positions:expr) => {
                let mut first = true;
                for p in $positions {
                    if !first {
                        yield Bytes::from_static(b", ");
                    }
                    first = false;
                    report_position!(p);
                }
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
                    format!("{}", err.user_error_message())
                        .replace("\"", "")
                        .replace("\n", "; ")
                );
                yield Bytes::from_static(br#"""#);
                if let SystemResolutionError::Validation(err) = err {
                    yield Bytes::from_static(br#", "locations": ["#);
                    report_positions!(err.positions());
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

fn create_system_resolver(claypot_file: &str) -> Result<SystemResolver, SystemLoadingError> {
    if !Path::new(&claypot_file).exists() {
        return Err(SystemLoadingError::FileNotFound(claypot_file.to_string()));
    }
    match File::open(claypot_file) {
        Ok(file) => {
            let claypot_file_buffer = BufReader::new(file);

            SystemLoader::load(claypot_file_buffer)
        }
        Err(e) => Err(SystemLoadingError::FileOpen(claypot_file.into(), e)),
    }
}

pub fn create_system_resolver_from_serialized_bytes(
    bytes: Vec<u8>,
) -> Result<SystemResolver, SystemLoadingError> {
    SystemLoader::load_from_bytes(bytes)
}

pub fn create_system_resolver_or_exit(claypot_file: &str) -> SystemResolver {
    match create_system_resolver(claypot_file) {
        Ok(system_resolver) => system_resolver,
        Err(error) => {
            println!("{}", error);
            exit(1);
        }
    }
}

/// Initializes logging for resolver.
pub fn init() {
    super::logging_tracing::init()
}
