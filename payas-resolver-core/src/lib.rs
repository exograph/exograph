pub mod access_solver;
pub mod column_path_util;
mod operation_payload;
mod query_response;
pub mod request_context;
pub mod validation;

use std::{future::Future, pin::Pin};

use maybe_owned::MaybeOwned;
pub use operation_payload::OperationsPayload;
pub use query_response::{QueryResponse, QueryResponseBody};

pub type ResolveOperationFn<'r> = Box<
    dyn Fn(
            OperationsPayload,
            MaybeOwned<'r, request_context::RequestContext<'r>>,
        ) -> Pin<
            Box<
                dyn Future<
                        Output = Result<
                            Vec<(String, QueryResponse)>,
                            Box<dyn std::error::Error + Send + Sync>,
                        >,
                    >
                    + 'r
                    + Send,
            >,
        >
        + 'r
        + Send
        + Sync,
>;
