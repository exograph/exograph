// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use indexmap::IndexMap;

use common::context::RequestContext;
use common::value::Val;
use core_resolver::{
    QueryResponse, QueryResponseBody,
    access_solver::{AccessSolver, AccessSolverError},
    context_extractor::ContextExtractor,
    exograph_execute_query,
    system_resolver::{ExographExecuteQueryFn, GraphQLSystemResolver},
    validation::field::ValidatedField,
};

use deno_graphql_model::{
    module::{Argument, ModuleMethod},
    subsystem::DenoSubsystem,
    types::{ModuleCompositeType, ModuleOperationReturnType, ModuleTypeKind},
};

use exo_deno::{Arg, deno_executor_pool::DenoScriptDefn};
use futures::StreamExt;

use crate::{
    DenoSubsystemResolver, deno_execution_error::DenoExecutionError,
    exo_execution::ExoCallbackProcessor, module_access_predicate::ModuleAccessPredicate,
};

use std::collections::HashMap;

pub struct DenoOperation<'a> {
    pub method: &'a ModuleMethod,
    pub field: &'a ValidatedField,
    pub request_context: &'a RequestContext<'a>,
    pub subsystem_resolver: &'a DenoSubsystemResolver,
    pub system_resolver: &'a GraphQLSystemResolver,
}

impl DenoOperation<'_> {
    pub async fn execute(&self) -> Result<QueryResponse, DenoExecutionError> {
        let access_predicate = self.compute_module_access_predicate().await?;

        if !access_predicate {
            return Err(DenoExecutionError::Authorization);
        }

        self.resolve_deno().await
    }

    async fn compute_module_access_predicate(&self) -> Result<bool, AccessSolverError> {
        match &self.method.return_type {
            ModuleOperationReturnType::Own(return_type) => {
                let subsystem = &self.subsystem();
                let return_type = return_type.typ(&subsystem.module_types);

                let type_level_access = match &return_type.kind {
                    ModuleTypeKind::Primitive | ModuleTypeKind::Injected => true,
                    ModuleTypeKind::Composite(ModuleCompositeType { access, .. }) => subsystem
                        .solve(self.request_context, None, &access.value)
                        .await?
                        .map(|r| matches!(r.0, ModuleAccessPredicate::True))
                        .resolve(),
                };

                let method_level_access = subsystem
                    .solve(self.request_context, None, &self.method.access.value)
                    .await?
                    .map(|r| r.0)
                    .resolve();

                // deny if either access check fails
                Ok(type_level_access
                    && !matches!(method_level_access, ModuleAccessPredicate::False))
            }
            ModuleOperationReturnType::Foreign(_) => {
                // For foreign types, Deno doesn't impose its own access control The associated code
                // may impose any required access control and in (typical) case of using
                // Exograph.executeQuery(), that itself will apply the necessary access control
                Ok(true)
            }
        }
    }

    async fn resolve_deno(&self) -> Result<QueryResponse, DenoExecutionError> {
        let subsystem = &self.subsystem();
        let script = &subsystem.scripts[self.method.script];

        let exograph_execute_query: &ExographExecuteQueryFn =
            exograph_execute_query!(self.system_resolver, self.request_context);

        let arg_sequence: Vec<Arg> = self.construct_arg_sequence().await?;

        let callback_processor = ExoCallbackProcessor {
            exograph_execute_query,
            exograph_proceed: None,
        };

        let deserialized: DenoScriptDefn = serde_json::from_slice(&script.script).unwrap();

        let (result, response) = self
            .subsystem_resolver
            .executor
            .execute_and_get_r(
                &script.path,
                deserialized,
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
    field_args: &IndexMap<String, Val>,
    args: &[Argument],
    system: &'a DenoSubsystem,
    request_context: &'a RequestContext<'a>,
) -> Result<Vec<Arg>, DenoExecutionError> {
    let mapped_args: HashMap<String, serde_json::Value> = field_args
        .iter()
        .map(|(module_name, module_value)| {
            (
                module_name.as_str().to_owned(),
                module_value.clone().try_into().unwrap(),
            )
        })
        .collect::<HashMap<_, _>>();

    futures::stream::iter(args.iter())
        .then(|arg| async {
            if arg.is_injected {
                // handle injected arguments

                let arg_type = &system.module_types[*arg.type_id.innermost()];

                // what kind of injected argument is it?
                // first check if it's a context
                if let Some(context) = system
                    .contexts
                    .iter()
                    .map(|(_, context)| context)
                    .find(|context| context.name == arg_type.name)
                {
                    // this argument is a context, get the value of the context and give it as an argument
                    let context_value = system
                        .extract_context(request_context, &arg_type.name)
                        .await?
                        .unwrap_or_else(|| {
                            panic!(
                                "Could not get context `{}` from request context",
                                &context.name
                            )
                        });
                    Ok(Arg::Serde(context_value.try_into().unwrap()))
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
