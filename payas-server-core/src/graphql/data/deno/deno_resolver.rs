use async_graphql_value::ConstValue;
use futures::FutureExt;
use futures::StreamExt;
use std::collections::HashMap;

use serde_json::{Map, Value};

use payas_deno::Arg;
use payas_model::model::operation::OperationReturnType;
use payas_model::model::service::{Argument, ServiceMethod, ServiceMethodType};
use payas_model::model::{GqlCompositeType, GqlCompositeTypeKind, GqlTypeKind};

use crate::graphql::data::access_solver;
use crate::graphql::data::operation_mapper::DenoOperation;

use crate::graphql::data::deno::{ClayCallbackProcessor, FnClaytipExecuteQuery};
use crate::graphql::execution::query_response::{QueryResponse, QueryResponseBody};
use crate::graphql::execution_error::{ExecutionError, ServiceExecutionError};
use crate::graphql::request_context::RequestContext;

use crate::graphql::{execution::system_context::SystemContext, validation::field::ValidatedField};

use payas_sql::{AbstractPredicate, Predicate};

impl DenoOperation {
    pub async fn execute<'a>(
        &self,
        field: &'a ValidatedField,
        system_context: &'a SystemContext,
        request_context: &'a RequestContext<'a>,
    ) -> Result<QueryResponse, ExecutionError> {
        let method = &system_context.system.methods[self.0];

        let access_predicate = compute_service_access_predicate(
            &method.return_type,
            method,
            system_context,
            request_context,
        )
        .await;

        if access_predicate == &Predicate::False {
            return Err(ExecutionError::Authorization);
        }

        resolve_deno(
            method,
            field,
            super::claytip_execute_query!(system_context, request_context),
            system_context,
            request_context,
        )
        .await
    }
}

pub async fn compute_service_access_predicate<'a>(
    return_type: &OperationReturnType,
    method: &'a ServiceMethod,
    system_context: &'a SystemContext,
    request_context: &'a RequestContext<'a>,
) -> &'a Predicate<'a> {
    let return_type = return_type.typ(&system_context.system);

    let type_level_access = match &return_type.kind {
        GqlTypeKind::Primitive => Predicate::True,
        GqlTypeKind::Composite(GqlCompositeType {
            access,
            kind: GqlCompositeTypeKind::NonPersistent,
            ..
        }) => {
            let access_expr = match &method.operation_kind {
                ServiceMethodType::Query(_) => &access.read, // query
                ServiceMethodType::Mutation(_) => &access.creation, // mutation
            };

            access_solver::solve_access(access_expr, request_context, &system_context.system).await
        }
        _ => panic!(),
    };

    let method_access_expr = match &method.operation_kind {
        ServiceMethodType::Query(_) => &method.access.read, // query
        ServiceMethodType::Mutation(_) => &method.access.creation, // mutation
    };

    let method_level_access =
        access_solver::solve_access(method_access_expr, request_context, &system_context.system)
            .await;

    let method_level_access = method_level_access.predicate();

    if matches!(type_level_access, AbstractPredicate::False)
        || matches!(method_level_access, Predicate::False)
    {
        &Predicate::False // deny if either access check fails
    } else {
        &Predicate::True
    }
}

pub async fn construct_arg_sequence(
    field_args: &HashMap<String, ConstValue>,
    args: &[Argument],
    system_context: &SystemContext,
    request_context: &RequestContext<'_>,
) -> Result<Vec<Arg>, ServiceExecutionError> {
    let system = &system_context.system;
    let mapped_args = field_args
        .iter()
        .map(|(gql_name, gql_value)| {
            (
                gql_name.as_str().to_owned(),
                gql_value.clone().into_json().unwrap(),
            )
        })
        .collect::<HashMap<_, _>>();

    futures::stream::iter(args.iter())
        .then(|arg| async {
            if arg.is_injected {
                // handle injected arguments

                let arg_type = &system.types[arg.type_id];

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
                        .extract_context(context)
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
                Err(ServiceExecutionError::InvalidArgument(arg.name.clone()))
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
    claytip_execute_query: Option<&'a FnClaytipExecuteQuery<'a>>,
    system_context: &SystemContext,
    request_context: &RequestContext<'_>,
) -> Result<QueryResponse, ExecutionError> {
    let script = &system_context.system.deno_scripts[method.script];

    // construct a sequence of arguments to pass to the Deno method
    let arg_sequence: Vec<Arg> = construct_arg_sequence(
        &field.arguments,
        &method.arguments,
        system_context,
        request_context,
    )
    .await?;

    let callback_processor = ClayCallbackProcessor {
        claytip_execute_query,
        claytip_proceed: None,
    };

    let (result, response) = system_context
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
        .map_err(ServiceExecutionError::Deno)?;

    Ok(QueryResponse {
        body: QueryResponseBody::Json(result),
        headers: response.map(|r| r.headers).unwrap_or_default(),
    })
}
