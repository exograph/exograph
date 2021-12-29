use async_graphql_parser::{types::Field, Positioned};
use async_recursion::async_recursion;
use futures::FutureExt;
use payas_deno::{
    Arg, FnClaytipExecuteQuery, FnClaytipInterceptorGetName, FnClaytipInterceptorProceed,
};
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
/// We want to execute the interceptors in the followiong order.

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
/// ```ingore
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
        // TODO: This block is duplicate of that from resolve_deno()

        //
        // FIXME the claytip_execute_query argument is the same for all invocations of execute_interceptor.
        // however the following doesn't work:
        //
        /////////////////////////////////////
        //
        //  let claytip_execute_query = &move |query_string: String, variables: Option<serde_json::Map<String, Value>>| {
        //      Box::pin(async move {
        //          let result = query_context
        //              .executor
        //              .execute_with_request_context(
        //                  None,
        //                  &query_string,
        //                  variables.as_ref(),
        //                  query_context.request_context.clone()
        //              )
        //              .await?
        //              .into_iter()
        //              .map(|(name, response)| (name, response.to_json().unwrap()) )
        //              .collect::<Map<_,_>>();

        //          Ok(serde_json::Value::Object(result))
        //      })
        //  };
        //  execute_interceptor(operation_name, before_interceptor, query_context, Some(&claytip_execute_query))
        //
        /////////////////////////////////////
        //
        //  rustc doesn't seem to be able to infer the associated type Output of claytip_execute_query's impl Future type unless it's drectly inlined
        //

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
                        Some(&move |query_string: String, variables: Option<serde_json::Map<String, Value>>| {
                            async move {
                                let result = query_context
                                    .executor
                                    .execute_with_request_context(
                                        None,
                                        &query_string,
                                        variables.as_ref(),
                                        query_context.request_context.clone()
                                    )
                                    .await?
                                    .into_iter()
                                    .map(|(name, response)| (name, response.to_json().unwrap()) )
                                    .collect::<Map<_,_>>();

                                Ok(serde_json::Value::Object(result))
                            }.boxed_local()
                        }),
                        Some(&|| {
                            (*operation_name).to_owned()
                        }), None
                    ).await?;
                }
                let res = core.execute(field, query_context).await?;
                for after_interceptor in after {
                    execute_interceptor(
                        after_interceptor,
                        query_context,
                        Some(&move |query_string: String, variables: Option<serde_json::Map<String, Value>>| {
                            async move {
                                let result = query_context
                                    .executor
                                    .execute_with_request_context(
                                        None,
                                        &query_string,
                                        variables.as_ref(),
                                        query_context.request_context.clone()
                                    )
                                    .await?
                                    .into_iter()
                                    .map(|(name, response)| (name, response.to_json().unwrap()) )
                                    .collect::<Map<_,_>>();

                                Ok(serde_json::Value::Object(result))
                            }.boxed_local()
                        }), Some(&|| {
                            (*operation_name).to_owned()
                        }), None
                    ).await?;
                }

                Ok(res)
            }
            &InterceptedOperation::Around {
                operation_name,
                ref core,
                interceptor,
            } => {
                let res = execute_interceptor(
                    interceptor,
                    query_context,
                    Some(&move |query_string: String, variables: Option<serde_json::Map<String, Value>>| {
                        async move {
                            let result = query_context
                                .executor
                                .execute_with_request_context(
                                    None,
                                    &query_string,
                                    variables.as_ref(),
                                    query_context.request_context.clone()
                                )
                                .await?
                                .into_iter()
                                .map(|(name, response)| (name, response.to_json().unwrap()) )
                                .collect::<Map<_,_>>();

                            Ok(serde_json::Value::Object(result))
                        }.boxed_local()
                    }),
                    Some(&|| {
                        operation_name.to_owned()
                    }),
                    Some(&|| {
                            async move {
                                core.execute(field, query_context)
                                    .await
                                    .map(|response| match response {
                                        QueryResponse::Json(json) => json,
                                        QueryResponse::Raw(string) => match string {
                                            Some(string) => serde_json::Value::String(string),
                                            None => serde_json::Value::Null,
                                        },
                                    })
                            }.boxed_local()
                        }),
                ).await?;
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
    claytip_get_interceptor: Option<&'a FnClaytipInterceptorGetName<'a>>,
    claytip_proceed_operation: Option<&'a FnClaytipInterceptorProceed<'a>>,
) -> Result<serde_json::Value> {
    let path = &interceptor.module_path;
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
        .preload_module(path, 10)
        .await;

    query_context
        .executor
        .deno_execution
        .execute_function_with_shims(
            path,
            &interceptor.name,
            arg_sequence,
            claytip_execute_query,
            claytip_get_interceptor,
            claytip_proceed_operation,
        )
        .await
}
