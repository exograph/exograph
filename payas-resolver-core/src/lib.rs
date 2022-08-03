pub mod access_solver;
pub mod column_path_util;
mod operation_payload;
mod query_response;
pub mod request_context;
pub mod validation;

use std::{future::Future, pin::Pin};

pub use operation_payload::OperationsPayload;
pub use query_response::{QueryResponse, QueryResponseBody};

pub type ResolveFn<'s, 'r> = Box<
    dyn Fn(
            OperationsPayload,
            &'r request_context::RequestContext<'r>,
        ) -> Pin<
            Box<
                dyn Future<
                        Output = Result<Vec<(String, QueryResponse)>, Box<dyn std::error::Error>>,
                    >
                    + 'r
                    + Send,
            >,
        >
        + 's
        + Send
        + Sync,
>;
