// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use indexmap::IndexMap;

use core_plugin_interface::core_resolver::{
    access_solver::{AccessSolver, AccessSolverError},
    context::RequestContext,
    context_extractor::ContextExtractor,
    exograph_execute_query,
    system_resolver::{ExographExecuteQueryFn, SystemResolver},
    validation::field::ValidatedField,
    value::Val,
    QueryResponse, QueryResponseBody,
};

use deno_model::{
    module::{Argument, ModuleMethod},
    subsystem::DenoSubsystem,
    types::{ModuleCompositeType, ModuleTypeKind},
};

use exo_deno::{deno_executor_pool::DenoScriptDefn, Arg};
use futures::StreamExt;

use crate::{
    deno_execution_error::DenoExecutionError, exo_execution::ExoCallbackProcessor,
    module_access_predicate::ModuleAccessPredicate, plugin::DenoSubsystemResolver,
};

use std::collections::HashMap;

pub struct DenoOperation<'a> {
    pub method: &'a ModuleMethod,
    pub field: &'a ValidatedField,
    pub request_context: &'a RequestContext<'a>,
    pub subsystem_resolver: &'a DenoSubsystemResolver,
    pub system_resolver: &'a SystemResolver,
}

impl<'a> DenoOperation<'a> {
    pub async fn execute(&self) -> Result<QueryResponse, DenoExecutionError> {
        let access_predicate = self.compute_module_access_predicate().await?;

        if !access_predicate {
            return Err(DenoExecutionError::Authorization);
        }

        self.resolve_deno().await
    }

    async fn compute_module_access_predicate(&self) -> Result<bool, AccessSolverError> {
        let subsystem = &self.subsystem();
        let return_type = self.method.return_type.typ(&subsystem.module_types);

        let type_level_access = match &return_type.kind {
            ModuleTypeKind::Primitive => true,
            ModuleTypeKind::Composite(ModuleCompositeType { access, .. }) => subsystem
                .solve(self.request_context, None, &access.value)
                .await?
                .map(|r| matches!(r.0, ModuleAccessPredicate::True))
                .unwrap_or(false),
        };

        let method_level_access = subsystem
            .solve(self.request_context, None, &self.method.access.value)
            .await?
            .map(|r| r.0)
            .unwrap_or(ModuleAccessPredicate::False);

        // deny if either access check fails
        Ok(!(matches!(type_level_access, false)
            || matches!(method_level_access, ModuleAccessPredicate::False)))
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
    let mapped_args = field_args
        .iter()
        .map(|(module_name, module_value)| {
            (
                module_name.as_str().to_owned(),
                module_value.clone().into_json().unwrap(),
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
                    Ok(Arg::Serde(context_value.into_json().unwrap()))
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
