use async_graphql_parser::{types::Field, Positioned};
use async_recursion::async_recursion;
use futures::FutureExt;
use payas_deno::{Arg, FnClaytipExecuteQuery, FnClaytipInterceptorProceed};
use payas_model::model::interceptor::{Interceptor, InterceptorKind};

use crate::execution::query_context::{QueryContext, QueryResponse};
use anyhow::{bail, Result};
use serde_json::{Map, Value};

/// Determine the order and nesting for interceptors.
///
/// TODO: Implement this scheme
///
/// The core idea (matches that in AspectJ):
/// - execute all the before interceptors prior to the operation, and all the after interceptors post the operation.
/// - a before interceptor defined earlier has a higher priority; it is the opposite for the after interceptors.
/// - an around interceptor defined earlier has a higher priority.
/// - all before/after interceptors defined earlier than an around interceptor execute by the time the around interceptor is executed.
///
/// Note that even for intra-service interceptors, this ides still holds true. All we need a preprocessing step to flatten the interceptors
/// to put the higher priority service's interceptors first.
///
/// Example: A service is set up with multiple interceptors in the following order (and identical
/// interceptor expressions):
///
/// ```ignore
/// @before 1
/// @before 2
/// @after  3   (has higher precedence than around 1, so must execute prior to finishing around 1)
/// @around 1
///
/// @before 3   (has higher precedence than around 2, so must execute prior to starting around 2)
/// @after  4   (has higher precedence than around 2, so must execute prior to finishing around 2)
/// @around 2
///
/// @before 4   (even when defined after around 2, must execute before the operation and after around 2 started (which has higher precedence than before 4))
///
/// @after  1   (has higher precedence than after 2, so must execute once after 2 finishes)
/// @after  2
/// ```
///
/// We want to execute the interceptors in the following order.

///
/// ```ignore
/// <before 1/>
/// <before 2/>
/// <around 1>
///     <before 3/>
///     <around 2>
///         <before 4/>
///         <OPERATION>
///         <after 4/>
///     </around 2>
///     <after 3/>
/// </around 1>
/// <after 2/>
/// <after 1/>
/// ```
///
/// Will translate to:
///
/// ```ignore
/// InterceptedOperation::Intercepted (
///     before: [
///         Interception::NonProceedingInterception(before 1)
///         Interception::NonProceedingInterception(before 2)
///     ],
///     core: Interception::ProceedingInterception(around 1, InterceptionChain(
///         Interception::NonProceedingInterception(before 3)
///         Interception::ProceedingInterception(around 2, InterceptionChain(
///             Interception::NonProceedingInterception(before 4),
///             Interception::Operation(OPERATION),
///             Interception::NonProceedingInterception(after 1)
///         )),
///         Interception::NonProceedingInterception(after 2)
///     )),
///     after: [
///         Interception::NonProceedingInterception(after 3)
///         Interception::NonProceedingInterception(after 4)
///     ]
/// )
/// ```
use super::operation_mapper::OperationResolverResult;

#[allow(clippy::large_enum_variant)]
pub enum InterceptedOperation<'a> {
    // around
    Intercepted {
        operation_name: &'a str,
        before: Vec<&'a Interceptor>,
        core: Box<InterceptedOperation<'a>>,
        after: Vec<&'a Interceptor>,
    },
    Around {
        operation_name: &'a str,
        core: Box<InterceptedOperation<'a>>,
        interceptor: &'a Interceptor,
    },
    // query/mutation
    Plain {
        resolver_result: OperationResolverResult<'a>,
    },
}

// this is a macro because rustc doesn't seem to be able to infer the associated type of
// the following future closure unless it's directly inlined
macro_rules! claytip_execute_query {
    ($query_context:ident) => {
        Some(
            &move |query_string: String, variables: Option<serde_json::Map<String, Value>>| {
                async move {
                    let result = $query_context
                        .executor
                        .execute_with_request_context(
                            None,
                            &query_string,
                            variables.as_ref(),
                            $query_context.request_context.clone(),
                        )
                        .await?
                        .into_iter()
                        .map(|(name, response)| (name, response.to_json().unwrap()))
                        .collect::<Map<_, _>>();

                    Ok(serde_json::Value::Object(result))
                }
                .boxed_local()
            },
        )
    };
}

impl<'a> InterceptedOperation<'a> {
    pub fn new(
        operation_name: &'a str,
        resolver_result: OperationResolverResult<'a>,
        interceptors: Vec<&'a Interceptor>,
    ) -> Self {
        if interceptors.is_empty() {
            Self::Plain { resolver_result }
        } else {
            let mut before = Vec::new();
            let mut after = Vec::new();
            let mut around = vec![];

            interceptors
                .into_iter()
                .for_each(|interceptor| match interceptor.interceptor_kind {
                    InterceptorKind::Before => before.push(interceptor),
                    InterceptorKind::After => after.push(interceptor),
                    InterceptorKind::Around => around.push(interceptor),
                });

            let core = Box::new(InterceptedOperation::Plain { resolver_result });

            let core = around.into_iter().fold(core, |core, interceptor| {
                Box::new(InterceptedOperation::Around {
                    operation_name,
                    core,
                    interceptor,
                })
            });

            Self::Intercepted {
                operation_name,
                before,
                core,
                after,
            }
        }
    }

    #[async_recursion(?Send)]
    pub async fn execute(
        &'a self,
        field: &'a Positioned<Field>,
        query_context: &'a QueryContext<'a>,
    ) -> Result<QueryResponse> {
        match self {
            InterceptedOperation::Intercepted {
                operation_name,
                before,
                core,
                after,
            } => {
                for before_interceptor in before {
                    execute_interceptor(
                        before_interceptor,
                        query_context,
                        claytip_execute_query!(query_context),
                        Some(operation_name.to_string()),
                        None,
                    )
                    .await?;
                }
                let res = core.execute(field, query_context).await?;
                for after_interceptor in after {
                    execute_interceptor(
                        after_interceptor,
                        query_context,
                        claytip_execute_query!(query_context),
                        Some(operation_name.to_string()),
                        None,
                    )
                    .await?;
                }

                Ok(res)
            }

            InterceptedOperation::Around {
                operation_name,
                core,
                interceptor,
            } => {
                let res = execute_interceptor(
                    interceptor,
                    query_context,
                    claytip_execute_query!(query_context),
                    Some(operation_name.to_string()),
                    Some(&|| {
                        async move {
                            core.execute(field, query_context).await.map(
                                |response| match response {
                                    QueryResponse::Json(json) => json,
                                    QueryResponse::Raw(string) => match string {
                                        Some(string) => serde_json::Value::String(string),
                                        None => serde_json::Value::Null,
                                    },
                                },
                            )
                        }
                        .boxed_local()
                    }),
                )
                .await?;
                match res {
                    serde_json::Value::String(value) => Ok(QueryResponse::Raw(Some(value))),
                    _ => Ok(QueryResponse::Json(res)),
                }
            }

            InterceptedOperation::Plain { resolver_result } => {
                resolver_result.execute(field, query_context).await
            }
        }
    }
}

async fn execute_interceptor<'a>(
    interceptor: &'a Interceptor,
    query_context: &'a QueryContext<'a>,

    claytip_execute_query: Option<&'a FnClaytipExecuteQuery<'a>>,
    claytip_get_interceptor: Option<String>,
    claytip_proceed_operation: Option<&'a FnClaytipInterceptorProceed<'a>>,
) -> Result<serde_json::Value> {
    let script = &query_context.executor.system.deno_scripts[interceptor.script];
    let arg_sequence = interceptor
        .arguments
        .iter()
        .map(|arg| {
            let arg_type = &query_context.executor.system.types[arg.type_id];

            if arg_type.name == "Operation" || arg_type.name == "ClaytipInjected" {
                // TODO: Change this to supply a shim if the arg_type is one of the shimmable types
                Ok(Arg::Shim(arg_type.name.clone()))
            } else {
                bail!("Invalid argument type {}", arg_type.name)
            }
        })
        .collect::<Result<Vec<_>>>()?;

    query_context
        .executor
        .deno_execution
        .preload_module(&script.path, &script.script, 1)
        .await?;

    query_context
        .executor
        .deno_execution
        .execute_function_with_shims(
            &script.path,
            &script.script,
            &interceptor.name,
            arg_sequence,
            claytip_execute_query,
            claytip_get_interceptor,
            claytip_proceed_operation,
        )
        .await
}
