use async_graphql_value::indexmap::IndexMap;
use async_graphql_value::ConstValue;
use futures::StreamExt;
use payas_core_resolver::request_context::RequestContext;
use payas_core_resolver::system_resolver::ClaytipExecuteQueryFn;
use payas_core_resolver::system_resolver::SystemResolver;
use payas_core_resolver::ResolveOperationFn;
use payas_deno_model::model::ModelDenoSystem;
use payas_deno_model::service::Argument;
use std::collections::HashMap;

use payas_core_resolver::validation::field::ValidatedField;
use payas_deno::Arg;
use payas_deno_model::service::ServiceMethod;
use payas_deno_model::types::{ServiceCompositeType, ServiceTypeKind};

use crate::access_solver;
use crate::clay_execution::ClayCallbackProcessor;
use crate::deno_execution_error::DenoExecutionError;
use crate::plugin::DenoSubsystemResolver;
use crate::service_access_predicate::ServiceAccessPredicate;

use payas_core_resolver::{QueryResponse, QueryResponseBody};

use payas_core_resolver::claytip_execute_query;

pub struct DenoOperation<'a> {
    pub method: &'a ServiceMethod,
    pub field: &'a ValidatedField,
    pub request_context: &'a RequestContext<'a>,
    pub subsystem_resolver: &'a DenoSubsystemResolver,
    pub system_resolver: &'a SystemResolver,
}

impl<'a> DenoOperation<'a> {
    pub async fn execute(&self) -> Result<QueryResponse, DenoExecutionError> {
        let access_predicate = self.compute_service_access_predicate().await;

        if !access_predicate {
            return Err(DenoExecutionError::Authorization);
        }

        self.resolve_deno().await
    }

    async fn compute_service_access_predicate(&self) -> bool {
        let subsystem = &self.subsystem();
        let return_type = self.method.return_type.typ(&subsystem.service_types);
        let resolve = &self.system_resolver.resolve_operation_fn();

        let type_level_access = match &return_type.kind {
            ServiceTypeKind::Primitive => true,
            ServiceTypeKind::Composite(ServiceCompositeType { access, .. }) => {
                let access_expr = &access.value;

                access_solver::solve_access(access_expr, self.request_context, subsystem, resolve)
                    .await
                    .into()
            }
        };

        let method_access_expr = &self.method.access.value;

        let method_level_access = access_solver::solve_access(
            method_access_expr,
            self.request_context,
            subsystem,
            resolve,
        )
        .await;

        let method_level_access = method_level_access;

        // deny if either access check fails
        !(matches!(type_level_access, false)
            || matches!(method_level_access, ServiceAccessPredicate::False))
    }

    async fn resolve_deno(&self) -> Result<QueryResponse, DenoExecutionError> {
        let subsystem = &self.subsystem();
        let script = &subsystem.scripts[self.method.script];

        let claytip_execute_query: &ClaytipExecuteQueryFn = claytip_execute_query!(
            self.system_resolver.resolve_operation_fn(),
            self.request_context
        );

        let arg_sequence: Vec<Arg> = self.construct_arg_sequence().await?;

        let callback_processor = ClayCallbackProcessor {
            claytip_execute_query: claytip_execute_query,
            claytip_proceed: None,
        };

        let (result, response) = self
            .subsystem_resolver
            .executor
            .execute_and_get_r(
                &script.path,
                &script.script,
                &self.method.name,
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

    pub async fn construct_arg_sequence(&self) -> Result<Vec<Arg>, DenoExecutionError> {
        construct_arg_sequence(
            &self.field.arguments,
            &self.method.arguments,
            self.subsystem(),
            &self.system_resolver.resolve_operation_fn(),
            self.request_context,
        )
        .await
    }

    fn subsystem(&self) -> &ModelDenoSystem {
        &self.subsystem_resolver.subsystem
    }
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
