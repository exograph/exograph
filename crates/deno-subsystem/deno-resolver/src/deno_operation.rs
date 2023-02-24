use async_graphql_value::indexmap::IndexMap;
use async_graphql_value::ConstValue;

use core_plugin_interface::core_resolver::{
    access_solver::AccessSolver,
    claytip_execute_query,
    request_context::RequestContext,
    system_resolver::{ClaytipExecuteQueryFn, SystemResolver},
    validation::field::ValidatedField,
    QueryResponse, QueryResponseBody,
};

use deno_model::{
    service::{Argument, ServiceMethod},
    subsystem::DenoSubsystem,
    types::{ServiceCompositeType, ServiceTypeKind},
};

use futures::StreamExt;
use payas_deno::Arg;
use serde_json::Value;

use crate::{
    clay_execution::ClayCallbackProcessor, deno_execution_error::DenoExecutionError,
    plugin::DenoSubsystemResolver, service_access_predicate::ServiceAccessPredicate,
};

use std::collections::HashMap;

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

        let type_level_access = match &return_type.kind {
            ServiceTypeKind::Primitive => true,
            ServiceTypeKind::Composite(ServiceCompositeType { access, .. }) => subsystem
                .solve(self.request_context, &access.value)
                .await
                .0
                .into(),
        };

        let method_level_access = subsystem
            .solve(self.request_context, &self.method.access.value)
            .await
            .0;

        let method_level_access = method_level_access;

        // deny if either access check fails
        !(matches!(type_level_access, false)
            || matches!(method_level_access, ServiceAccessPredicate::False))
    }

    async fn resolve_deno(&self) -> Result<QueryResponse, DenoExecutionError> {
        let subsystem = &self.subsystem();
        let script = &subsystem.scripts[self.method.script];

        let claytip_execute_query: &ClaytipExecuteQueryFn =
            claytip_execute_query!(self.system_resolver, self.request_context);

        let arg_sequence: Vec<Arg> = self.construct_arg_sequence().await?;

        let callback_processor = ClayCallbackProcessor {
            claytip_execute_query,
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
            self.request_context,
        )
        .await
    }

    fn subsystem(&self) -> &DenoSubsystem {
        &self.subsystem_resolver.subsystem
    }
}

pub async fn construct_arg_sequence<'a>(
    field_args: &IndexMap<String, ConstValue>,
    args: &[Argument],
    system: &'a DenoSubsystem,
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

                let arg_type = &system.service_types[*arg.type_id.innermost()];

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
                    Ok(Arg::Serde(Value::Object(context_value)))
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
