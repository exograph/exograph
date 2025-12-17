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
        eprintln!("[DenoOperation] executing method '{}'", self.method.name);
        let access_predicate = self.compute_module_access_predicate().await?;

        if !access_predicate {
            eprintln!("[DenoOperation] access denied for '{}'", self.method.name);
            return Err(DenoExecutionError::Authorization);
        }

        self.resolve_deno().await
    }

    async fn compute_module_access_predicate(&self) -> Result<bool, AccessSolverError> {
        if self.request_context.is_internal() {
            eprintln!(
                "[DenoOperation] skipping access check for '{}' (internal request)",
                self.method.name
            );
            return Ok(true);
        }

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

                eprintln!(
                    "[DenoOperation] access check for '{}': type_level={}, method_level={:?}",
                    self.method.name, type_level_access, method_level_access
                );

                // deny if either access check fails
                let allow = type_level_access
                    && !matches!(method_level_access, ModuleAccessPredicate::False);
                eprintln!(
                    "[DenoOperation] access result for '{}': {}",
                    self.method.name, allow
                );
                Ok(allow)
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

        eprintln!(
            "[DenoOperation] constructing args for '{}'",
            self.method.name
        );
        let arg_sequence: Vec<Arg> = self.construct_arg_sequence().await?;
        eprintln!(
            "[DenoOperation] invoking '{}' (from '{}') with {} arg(s)",
            self.method.name,
            script.path,
            arg_sequence.len()
        );

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
                    eprintln!(
                        "[DenoOperation] attempting to extract context '{}' for argument '{}'",
                        context.name, arg.name
                    );
                    // this argument is a context, get the value of the context and give it as an argument
                    let context_value_result =
                        system.extract_context(request_context, &arg_type.name).await;
                    match context_value_result {
                        Ok(Some(context_value)) => {
                            eprintln!(
                                "[DenoOperation] injecting context '{}' for argument '{}'",
                                context.name, arg.name
                            );
                            Ok(Arg::Serde(context_value.try_into().unwrap()))
                        }
                        Ok(None) => {
                            eprintln!(
                                "[DenoOperation] context '{}' resolved to None for argument '{}'",
                                context.name, arg.name
                            );
                            Err(DenoExecutionError::Authorization)
                        }
                        Err(err) => {
                            eprintln!(
                                "[DenoOperation] failed to extract context '{}' for argument '{}': {:?}",
                                context.name, arg.name, err
                            );
                            Err(DenoExecutionError::Authorization)
                        }
                    }
                } else {
                    // not a context, assume it is a provided shim by the Deno executor
                    eprintln!(
                        "[DenoOperation] injecting shim '{}' for argument '{}'",
                        arg_type.name, arg.name
                    );
                    Ok(Arg::Shim(arg_type.name.clone()))
                }
            } else if let Some(val) = mapped_args.get(&arg.name) {
                // regular argument
                eprintln!(
                    "[DenoOperation] passing provided arg '{}' (type {:?})",
                    arg.name,
                    arg.type_id
                );
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
