use async_graphql_value::indexmap::IndexMap;
use async_graphql_value::ConstValue;
use futures::FutureExt;
use futures::StreamExt;
use payas_deno_model::model::ModelDenoSystem;
use payas_deno_model::operation::OperationReturnType;
use payas_resolver_core::request_context::RequestContext;
use payas_resolver_core::ResolveOperationFn;
use std::collections::HashMap;

use payas_deno::Arg;
use payas_deno_model::service::{Argument, ServiceMethod};
use payas_deno_model::types::{ServiceCompositeType, ServiceTypeKind};
use payas_resolver_core::validation::field::ValidatedField;

use crate::access_solver;
use crate::clay_execution::ClayCallbackProcessor;
use crate::service_access_predicate::ServiceAccessPredicate;

use super::deno_system_context::DenoSystemContext;

use payas_resolver_core::{QueryResponse, QueryResponseBody};

use super::DenoExecutionError;

pub struct DenoOperation<'a> {
    pub method: &'a ServiceMethod,
    pub field: &'a ValidatedField,
    pub request_context: &'a RequestContext<'a>,
}

impl<'a> DenoOperation<'a> {
    pub async fn execute(
        &self,
        deno_system_context: &DenoSystemContext<'a>,
    ) -> Result<QueryResponse, DenoExecutionError> {
        let access_predicate = compute_service_access_predicate(
            &self.method.return_type,
            self.method,
            deno_system_context,
            self.request_context,
        )
        .await;

        if !access_predicate {
            return Err(DenoExecutionError::Authorization);
        }

        resolve_deno(
            self.method,
            self.field,
            deno_system_context,
            self.request_context,
        )
        .await
    }
}

async fn compute_service_access_predicate<'a>(
    return_type: &OperationReturnType,
    method: &'a ServiceMethod,
    system_context: &DenoSystemContext<'a>,
    request_context: &'a RequestContext<'a>,
) -> bool {
    let return_type = return_type.typ(&system_context.system.service_types);
    let resolve = &system_context.resolve_operation_fn;

    let type_level_access = match &return_type.kind {
        ServiceTypeKind::Primitive => true,
        ServiceTypeKind::Composite(ServiceCompositeType { access, .. }) => {
            let access_expr = &access.value;

            access_solver::solve_access(
                access_expr,
                request_context,
                system_context.system,
                resolve,
            )
            .await
            .into()
        }
    };

    let method_access_expr = &method.access.value;

    let method_level_access = access_solver::solve_access(
        method_access_expr,
        request_context,
        system_context.system,
        resolve,
    )
    .await;

    let method_level_access = method_level_access;

    // deny if either access check fails
    !(matches!(type_level_access, false)
        || matches!(method_level_access, ServiceAccessPredicate::False))
}

pub async fn construct_arg_sequence<'a>(
    field_args: &IndexMap<String, ConstValue>,
    args: &[Argument],
    system: &'a ModelDenoSystem,
    resolve_query: &ResolveOperationFn<'a>,
    request_context: &'a RequestContext<'a>,
) -> Result<Vec<Arg>, DenoExecutionError> {
    let mapped_args = field_args
        .iter()
        .map(|(service_name, service_value)| {
            (
                service_name.as_str().to_owned(),
                service_value.clone().into_json().unwrap(),
            )
        })
        .collect::<HashMap<_, _>>();

    futures::stream::iter(args.iter())
        .then(|arg| async {
            if arg.is_injected {
                // handle injected arguments

                let arg_type = &system.service_types[arg.type_id];

                // what kind of injected argument is it?
                // first check if it's a context
                if let Some(context) = system
                    .contexts
                    .iter()
                    .map(|(_, context)| context)
                    .find(|context| context.name == arg_type.name)
                {
                    // this argument is a context, get the value of the context and give it as an argument
                    let context_value = request_context
                        .extract_context(context, resolve_query)
                        .await
                        .unwrap_or_else(|_| {
                            panic!(
                                "Could not get context `{}` from request context",
                                &context.name
                            )
                        });
                    Ok(Arg::Serde(context_value))
                } else {
                    // not a context, assume it is a provided shim by the Deno executor
                    Ok(Arg::Shim(arg_type.name.clone()))
                }
            } else if let Some(val) = mapped_args.get(&arg.name) {
                // regular argument
                Ok(Arg::Serde(val.clone()))
            } else {
                Err(DenoExecutionError::InvalidArgument(arg.name.clone()))
            }
        })
        .collect::<Vec<Result<_, _>>>()
        .await
        .into_iter()
        .collect::<Result<_, _>>()
}

async fn resolve_deno<'a>(
    method: &ServiceMethod,
    field: &ValidatedField,
    deno_system_context: &DenoSystemContext<'a>,
    request_context: &'a RequestContext<'a>,
) -> Result<QueryResponse, DenoExecutionError> {
    let script = &deno_system_context.system.scripts[method.script];

    let claytip_execute_query =
        super::claytip_execute_query!(deno_system_context.resolve_operation_fn, request_context);

    let arg_sequence: Vec<Arg> = construct_arg_sequence(
        &field.arguments,
        &method.arguments,
        deno_system_context.system,
        &deno_system_context.resolve_operation_fn,
        request_context,
    )
    .await?;

    let callback_processor = ClayCallbackProcessor {
        claytip_execute_query,
        claytip_proceed: None,
    };

    let (result, response) = deno_system_context
        .deno_execution_pool
        .execute_and_get_r(
            &script.path,
            &script.script,
            &method.name,
            arg_sequence,
            None,
            callback_processor,
        )
        .await
        .map_err(DenoExecutionError::Deno)?;

    Ok(QueryResponse {
        body: QueryResponseBody::Json(result),
        headers: response.map(|r| r.headers).unwrap_or_default(),
    })
}
