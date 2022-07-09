use std::collections::HashMap;

use crate::{
    deno_integration::{
        clay_execution::ClaytipMethodResponse, ClayCallbackProcessor, FnClaytipExecuteQuery,
        FnClaytipInterceptorProceed, InterceptedOperationInfo,
    },
    execution::system_context::QueryResponseBody,
    request_context::RequestContext,
};
use async_recursion::async_recursion;
use futures::FutureExt;
use payas_deno::Arg;
use payas_model::model::interceptor::{Interceptor, InterceptorKind};

use crate::{
    execution::system_context::{QueryResponse, SystemContext},
    validation::field::ValidatedField,
    OperationsPayload,
};
use anyhow::Result;
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
use super::operation_mapper::{construct_arg_sequence, OperationResolverResult};

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

    #[async_recursion]
    pub async fn execute(
        &'a self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
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
                        system_context,
                        request_context,
                        super::claytip_execute_query!(system_context, request_context),
                        Some(operation_name.to_string()),
                        field,
                        None,
                    )
                    .await?;
                }
                let res = core.execute(field, system_context, request_context).await?;
                for after_interceptor in after {
                    execute_interceptor(
                        after_interceptor,
                        system_context,
                        request_context,
                        super::claytip_execute_query!(system_context, request_context),
                        Some(operation_name.to_string()),
                        field,
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
                let (result, response) = execute_interceptor(
                    interceptor,
                    system_context,
                    request_context,
                    super::claytip_execute_query!(system_context, request_context),
                    Some(operation_name.to_string()),
                    field,
                    Some(&|| {
                        async move { core.execute(field, system_context, request_context).await }
                            .boxed()
                    }),
                )
                .await?;
                let body = match result {
                    serde_json::Value::String(value) => QueryResponseBody::Raw(Some(value)),
                    _ => QueryResponseBody::Json(result),
                };

                Ok(QueryResponse {
                    body,
                    headers: response.map(|r| r.headers).unwrap_or_default(),
                })
            }

            InterceptedOperation::Plain { resolver_result } => {
                resolver_result
                    .execute(field, system_context, request_context)
                    .await
            }
        }
    }
}

async fn execute_interceptor<'a>(
    interceptor: &'a Interceptor,
    system_context: &'a SystemContext,
    request_context: &'a RequestContext<'a>,
    claytip_execute_query: Option<&'a FnClaytipExecuteQuery<'a>>,
    operation_name: Option<String>,
    operation_query: &'a ValidatedField,
    claytip_proceed_operation: Option<&'a FnClaytipInterceptorProceed<'a>>,
) -> Result<(Value, Option<ClaytipMethodResponse>)> {
    let script = &system_context.system.deno_scripts[interceptor.script];

    let serialized_operation_query = serde_json::to_value(operation_query).unwrap();

    let arg_sequence: Vec<Arg> = construct_arg_sequence(
        &HashMap::new(),
        &interceptor.arguments,
        system_context,
        request_context,
    )
    .await?;

    let callback_processor = ClayCallbackProcessor {
        claytip_execute_query,
        claytip_proceed: claytip_proceed_operation,
    };

    system_context
        .deno_execution_pool
        .execute_and_get_r(
            &script.path,
            &script.script,
            &interceptor.name,
            arg_sequence,
            operation_name.map(|name| InterceptedOperationInfo {
                name,
                query: serialized_operation_query,
            }),
            callback_processor,
        )
        .await
}
