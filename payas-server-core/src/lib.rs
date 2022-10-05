/// Provides core functionality for handling incoming queries without depending
/// on any specific web framework.
///
/// The `resolve` function is responsible for doing the work, using information
/// extracted from an incoming request, and returning the response as a stream.
mod initialization_error;
mod logging_tracing;
mod root_resolver;
mod system_loader;

pub mod graphiql;
pub use root_resolver::create_system_resolver_from_serialized_bytes;
pub use root_resolver::create_system_resolver_or_exit;
pub use root_resolver::get_endpoint_http_path;
pub use root_resolver::get_playground_http_path;
pub use root_resolver::init;
pub use root_resolver::resolve;
