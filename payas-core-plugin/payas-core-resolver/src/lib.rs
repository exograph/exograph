mod field_resolver;
mod interception;
pub mod introspection;
mod operation_payload;
pub mod operation_resolver;
pub mod plugin;
mod query_response;
pub mod request_context;
pub mod system_resolver;
pub mod validation;

pub use field_resolver::FieldResolver;
pub use interception::InterceptedOperation;
use maybe_owned::MaybeOwned;
pub use operation_payload::OperationsPayload;
pub use query_response::{QueryResponse, QueryResponseBody};

use futures::future::BoxFuture;

pub type ResolveOperationFn<'r> = Box<
    dyn Fn(
            OperationsPayload,
            MaybeOwned<'r, request_context::RequestContext<'r>>,
        ) -> BoxFuture<
            'r,
            Result<Vec<(String, QueryResponse)>, Box<dyn std::error::Error + Send + Sync>>,
        >
        + 'r
        + Send
        + Sync,
>;
