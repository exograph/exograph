use async_recursion::async_recursion;
use futures::{future::BoxFuture, FutureExt};

use payas_deno_model::interceptor::{Interceptor, InterceptorKind};
use payas_resolver_core::{
    request_context::RequestContext, validation::field::ValidatedField, QueryResponse,
    QueryResponseBody,
};
use payas_resolver_deno::{
    claytip_execute_query, execute_interceptor, DenoExecutionError, DenoSystemContext,
};

use crate::graphql::execution::system_context::SystemContext;

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

pub type ResolveFieldFn<'a> = (dyn Fn(
    &'a ValidatedField,
    &'a RequestContext<'a>,
) -> BoxFuture<'a, Result<QueryResponse, DenoExecutionError>>
     + 'a
     + Send
     + Sync);

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
    Plain,
}

impl<'a> InterceptedOperation<'a> {
    pub fn new(operation_name: &'a str, interceptors: Vec<&'a Interceptor>) -> Self {
        if interceptors.is_empty() {
            Self::Plain
        } else {
            let mut before = vec![];
            let mut after = vec![];
            let mut around = vec![];

            interceptors
                .into_iter()
                .for_each(|interceptor| match interceptor.interceptor_kind {
                    InterceptorKind::Before => before.push(interceptor),
                    InterceptorKind::After => after.push(interceptor),
                    InterceptorKind::Around => around.push(interceptor),
                });

            let core = Box::new(InterceptedOperation::Plain);

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

    /// Execute the intercepted operation
    ///
    /// # Arguments
    /// * `resolve` - Function to resolve the operation. This needs to be evaluated in the context of the intercepted operation, since
    ///               the intercepted operation may be invoked with overridden context (privileged execution) and access control must
    ///               be evaluated considering the overridden context.
    #[async_recursion]
    pub async fn execute(
        &self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        deno_system_context: &DenoSystemContext<'a>,
        request_context: &'a RequestContext<'a>,
        resolve_field: &ResolveFieldFn<'a>,
    ) -> Result<QueryResponse, DenoExecutionError> {
        let system = &system_context.system;
        let deno_execution_pool = &deno_system_context.deno_execution_pool;
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
                        &system.deno_subsystem,
                        deno_execution_pool,
                        request_context,
                        claytip_execute_query!(
                            deno_system_context.resolve_operation_fn,
                            request_context
                        ),
                        operation_name.to_string(),
                        field,
                        None,
                        system_context.resolve_operation_fn(),
                    )
                    .await?;
                }
                let res = core
                    .execute(
                        field,
                        system_context,
                        deno_system_context,
                        request_context,
                        resolve_field,
                    )
                    .await?;
                for after_interceptor in after {
                    execute_interceptor(
                        after_interceptor,
                        &system.deno_subsystem,
                        deno_execution_pool,
                        request_context,
                        claytip_execute_query!(
                            deno_system_context.resolve_operation_fn,
                            request_context
                        ),
                        operation_name.to_string(),
                        field,
                        None,
                        system_context.resolve_operation_fn(),
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
                    &system.deno_subsystem,
                    deno_execution_pool,
                    request_context,
                    claytip_execute_query!(
                        deno_system_context.resolve_operation_fn,
                        request_context
                    ),
                    operation_name.to_string(),
                    field,
                    Some(&|| {
                        async move {
                            core.execute(
                                field,
                                system_context,
                                deno_system_context,
                                request_context,
                                resolve_field,
                            )
                            .await
                        }
                        .boxed()
                    }),
                    system_context.resolve_operation_fn(),
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

            InterceptedOperation::Plain => resolve_field(field, request_context).await,
        }
    }
}
