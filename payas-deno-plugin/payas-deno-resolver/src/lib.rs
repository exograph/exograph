pub use clay_execution::clay_config;
pub use deno_execution_error::DenoExecutionError;
pub use deno_operation::DenoOperation;
pub use deno_system_context::DenoSystemContext;
pub use interceptor_execution::execute_interceptor;
pub type ClayDenoExecutorPool = DenoExecutorPool<
    Option<InterceptedOperationInfo>,
    clay_execution::RequestFromDenoMessage,
    clay_execution::ClaytipMethodResponse,
>;

mod access_solver;
mod clay_execution;
mod claytip_ops;
mod deno_execution_error;
mod deno_operation;
mod deno_system_context;
mod interceptor_execution;
mod service_access_predicate;

use claytip_ops::InterceptedOperationInfo;
use payas_deno::DenoExecutorPool;

#[macro_export]
macro_rules! claytip_execute_query {
    ($resolve_query_fn:expr, $request_context:ident) => {
        &move |query_string: String,
               variables: Option<serde_json::Map<String, serde_json::Value>>,
               context_override: serde_json::Value| {
            use maybe_owned::MaybeOwned;
            let new_request_context = $request_context.with_override(context_override);
            async move {
                // execute query
                let result = $resolve_query_fn(
                    payas_core_resolver::OperationsPayload {
                        operation_name: None,
                        query: query_string,
                        variables,
                    },
                    MaybeOwned::Owned(new_request_context),
                )
                .await
                .map_err(|e| DenoExecutionError::Delegate(e))?;

                // collate result into a single QueryResponse

                // since query execution results in a Vec<(String, QueryResponse)>, we want to
                // extract and collect all HTTP headers generated in QueryResponses
                let headers = result
                    .iter()
                    .flat_map(|(_, response)| response.headers.clone())
                    .collect::<Vec<_>>();

                // generate the body
                let body = result
                    .into_iter()
                    .map(|(name, response)| (name, response.body.to_json().unwrap()))
                    .collect::<serde_json::Map<_, _>>();

                Ok(QueryResponse {
                    body: QueryResponseBody::Json(serde_json::Value::Object(body)),
                    headers,
                })
            }
            .boxed()
        }
    };
}
