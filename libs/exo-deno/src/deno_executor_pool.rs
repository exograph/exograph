// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use deno_core::{Extension, ModuleType, url::Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::{Arg, error::DenoError};

use super::{
    deno_actor::DenoActor,
    deno_executor::{CallbackProcessor, DenoExecutor},
    deno_module::{DenoModule, UserCode},
};

use std::fmt::Debug;

type DenoActorPoolMap<C, M, R> = HashMap<String, DenoActorPool<C, M, R>>;
type DenoActorPool<C, M, R> = Vec<DenoActor<C, M, R>>;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ResolvedModule {
    Module(String, ModuleType, Url, bool),
    Redirect(Url),
}

/// Serialized type for modules loaded during the build phase
#[derive(Serialize, Deserialize, Debug)]
pub struct DenoScriptDefn {
    pub modules: HashMap<Url, ResolvedModule>,
}

pub struct DenoExecutorConfig<C> {
    shims: Vec<(&'static str, &'static [&'static str])>,
    additional_code: Vec<&'static str>,
    explicit_error_class_name: Option<&'static str>,
    create_extensions: fn() -> Vec<Extension>,
    process_call_context: fn(&mut DenoModule, C) -> (),
}

impl<C> DenoExecutorConfig<C> {
    pub fn new(
        shims: Vec<(&'static str, &'static [&'static str])>,
        additional_code: Vec<&'static str>,
        explicit_error_class_name: Option<&'static str>,
        create_extensions: fn() -> Vec<Extension>,
        process_call_context: fn(&mut DenoModule, C) -> (),
    ) -> Self {
        Self {
            shims,
            additional_code,
            explicit_error_class_name,
            create_extensions,
            process_call_context,
        }
    }
}

/// DenoExecutorPool maintains a pool of `DenoActor`s for each module to delegate work to.
///
/// Calling `execute` will either select a free actor or allocate a new `DenoActor` to run the function on.
/// It will create a `DenoExecutor` with that actor and delegate the method execution to it.
///
/// The hierarchy of modules:
///
/// DenoExecutorPool -> DenoExecutor -> DenoActor -> DenoModule
///                  -> DenoExecutor -> DenoActor -> DenoModule
///                  -> DenoExecutor -> DenoActor -> DenoModule
///
/// # Type Parameters
/// - `C`: The type of the call context (for example, `Option<InterceptedOperationName>`). This object
///   is set into the `DenoModule`s GothamState and may be resolved synchronously or asynchronously.
/// - `M`: The type of the callback message.
/// - `R`: An opaque return type to also return from GothamStorage with each method execution. Useful for
///   returning out-of-band information that should not be a part of the return value.
pub struct DenoExecutorPool<C, M, R> {
    config: DenoExecutorConfig<C>,
    actor_pool_map: Arc<Mutex<DenoActorPoolMap<C, M, R>>>,
    return_type: PhantomData<R>,
}

impl<C: Sync + Send + Debug + 'static, M: Sync + Send + 'static, R: Sync + Send + Debug + 'static>
    DenoExecutorPool<C, M, R>
{
    pub fn new(
        shims: Vec<(&'static str, &'static [&'static str])>,
        additional_code: Vec<&'static str>,
        explicit_error_class_name: Option<&'static str>,
        create_extensions: fn() -> Vec<Extension>,
        process_call_context: fn(&mut DenoModule, C) -> (),
    ) -> Self {
        Self::new_from_config(DenoExecutorConfig::new(
            shims,
            additional_code,
            explicit_error_class_name,
            create_extensions,
            process_call_context,
        ))
    }

    pub fn new_from_config(config: DenoExecutorConfig<C>) -> Self {
        Self {
            config,
            actor_pool_map: Arc::new(Mutex::new(DenoActorPoolMap::default())),
            return_type: PhantomData,
        }
    }

    // Execute a method and obtain its result
    pub async fn execute(
        &self,
        script_path: &str,
        script: DenoScriptDefn,
        method_name: &str,
        arguments: Vec<Arg>,
        call_context: C,
        callback_processor: impl CallbackProcessor<M>,
    ) -> Result<Value, DenoError> {
        let (result, _) = self
            .execute_and_get_r(
                script_path,
                script,
                method_name,
                arguments,
                call_context,
                callback_processor,
            )
            .await?;
        Ok(result)
    }

    // execute(...), but also return R from Deno's GothamStorage
    pub async fn execute_and_get_r(
        &self,
        script_path: &str,
        script: DenoScriptDefn,
        method_name: &str,
        arguments: Vec<Arg>,
        call_context: C,
        callback_processor: impl CallbackProcessor<M>,
    ) -> Result<(Value, Option<R>), DenoError> {
        let executor = self.get_executor(script_path, script).await?;
        executor
            .execute(method_name, arguments, call_context, callback_processor)
            .await
    }

    // TODO: look at passing a fn pointer struct as an argument
    async fn get_executor(
        &self,
        script_path: &str,
        script: DenoScriptDefn,
    ) -> Result<DenoExecutor<C, M, R>, DenoError> {
        // find or allocate a free actor in our pool
        let actor = {
            let mut actor_pool_map = self.actor_pool_map.lock().await;
            let actor_pool = actor_pool_map.entry(script_path.to_string()).or_default();

            let free_actor = actor_pool.iter().find(|actor| !actor.is_busy());

            if let Some(actor) = free_actor {
                // found a free actor!
                actor.clone()
            } else {
                // no free actors; need to allocate a new DenoActor
                let new_actor = self.create_actor(script_path, script)?;

                actor_pool.push(new_actor.clone());
                new_actor
            }
        };

        Ok(DenoExecutor { actor })
    }

    fn create_actor(
        &self,
        script_path: &str,
        script: DenoScriptDefn,
    ) -> Result<DenoActor<C, M, R>, DenoError> {
        DenoActor::new(
            UserCode::LoadFromMemory {
                path: script_path.to_owned(),
                script,
            },
            self.config.shims.clone(),
            self.config.additional_code.clone(),
            self.config.create_extensions,
            self.config.explicit_error_class_name,
            self.config.process_call_context,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deno_module::Arg;
    use deno_core::ModuleSpecifier;
    use serde_json::Value;

    use futures::future::join_all;

    #[tokio::test]
    async fn test_actor_executor() {
        let module_path = "file://test_js/direct.js";
        let module_script = include_str!("test_js/direct.js").to_string();

        let executor_pool =
            DenoExecutorPool::<(), (), ()>::new(vec![], vec![], None, Vec::new, |_, _| {});

        let res = executor_pool
            .execute(
                module_path,
                DenoScriptDefn {
                    modules: vec![(
                        ModuleSpecifier::parse(module_path).unwrap(),
                        ResolvedModule::Module(
                            module_script,
                            ModuleType::JavaScript,
                            ModuleSpecifier::parse(module_path).unwrap(),
                            false,
                        ),
                    )]
                    .into_iter()
                    .collect(),
                },
                "addAndDouble",
                vec![Arg::Serde(2.into()), Arg::Serde(3.into())],
                (),
                (),
            )
            .await;

        assert_eq!(res.unwrap(), 10);
    }

    #[tokio::test]
    async fn test_actor_executor_concurrent() {
        let module_path = "file://test_js/direct.js";
        let module_script = include_str!("test_js/direct.js").to_string();

        let executor_pool = DenoExecutorPool::new(vec![], vec![], None, Vec::new, |_, _| {});

        let total_futures = 10;

        let mut handles = vec![];

        async fn execute_function(
            pool: &DenoExecutorPool<(), (), ()>,
            script_path: &str,
            script: String,
            method_name: &str,
            arguments: Vec<Arg>,
        ) -> Result<Value, DenoError> {
            pool.execute(
                script_path,
                DenoScriptDefn {
                    modules: vec![(
                        ModuleSpecifier::parse(script_path).unwrap(),
                        ResolvedModule::Module(
                            script,
                            ModuleType::JavaScript,
                            ModuleSpecifier::parse(script_path).unwrap(),
                            false,
                        ),
                    )]
                    .into_iter()
                    .collect(),
                    // npm_snapshot: None,
                },
                method_name,
                arguments,
                (),
                (),
            )
            .await
        }

        for _ in 1..=total_futures {
            let handle = execute_function(
                &executor_pool,
                module_path,
                module_script.clone(),
                "addAndDouble",
                vec![
                    Arg::Serde(Value::Number(4.into())),
                    Arg::Serde(Value::Number(2.into())),
                ],
            );

            handles.push(handle);
        }

        let result = join_all(handles)
            .await
            .iter()
            .filter(|res| res.as_ref().unwrap() == 12)
            .count();

        assert_eq!(result, total_futures);
    }
}
