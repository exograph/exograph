use std::{collections::HashMap, sync::Arc};

use deno_core::Extension;
use tokio::sync::Mutex;

use super::{
    deno_actor::DenoActor,
    deno_executor::DenoExecutor,
    deno_module::{DenoModule, DenoModuleSharedState, UserCode},
};
use anyhow::Result;

type DenoActorPoolMap<C, M> = HashMap<String, DenoActorPool<C, M>>;
type DenoActorPool<C, M> = Vec<DenoActor<C, M>>;

pub struct DenoExecutorConfig<C> {
    user_agent_name: &'static str,
    shims: Vec<(&'static str, &'static str)>,
    additional_code: Vec<&'static str>,
    explicit_error_class_name: Option<&'static str>,
    create_extensions: fn() -> Vec<Extension>,
    process_call_context: fn(&mut DenoModule, C) -> (),
    shared_state: DenoModuleSharedState,
}

impl<C> DenoExecutorConfig<C> {
    pub fn new(
        user_agent_name: &'static str,
        shims: Vec<(&'static str, &'static str)>,
        additional_code: Vec<&'static str>,
        explicit_error_class_name: Option<&'static str>,
        create_extensions: fn() -> Vec<Extension>,
        process_call_context: fn(&mut DenoModule, C) -> (),
        shared_state: DenoModuleSharedState,
    ) -> Self {
        Self {
            user_agent_name,
            shims,
            additional_code,
            explicit_error_class_name,
            create_extensions,
            process_call_context,
            shared_state,
        }
    }
}

pub struct DenoExecutorPool<C, M> {
    config: DenoExecutorConfig<C>,

    actor_pool_map: Arc<Mutex<DenoActorPoolMap<C, M>>>,
}

impl<C: Sync + Send + std::fmt::Debug + 'static, M: Sync + Send + 'static> DenoExecutorPool<C, M> {
    pub fn new(
        user_agent_name: &'static str,
        shims: Vec<(&'static str, &'static str)>,
        additional_code: Vec<&'static str>,
        explicit_error_class_name: Option<&'static str>,
        create_extensions: fn() -> Vec<Extension>,
        process_call_context: fn(&mut DenoModule, C) -> (),
        shared_state: DenoModuleSharedState,
    ) -> Self {
        Self::new_from_config(DenoExecutorConfig::new(
            user_agent_name,
            shims,
            additional_code,
            explicit_error_class_name,
            create_extensions,
            process_call_context,
            shared_state,
        ))
    }

    pub fn new_from_config(config: DenoExecutorConfig<C>) -> Self {
        Self {
            config,
            actor_pool_map: Arc::new(Mutex::new(DenoActorPoolMap::default())),
        }
    }

    // TODO: look at passing a fn pointer struct as an argument
    #[allow(clippy::too_many_arguments)]
    pub async fn get_executor(
        &self,
        script_path: &str,
        script: &str,
    ) -> Result<DenoExecutor<C, M>> {
        // find or allocate a free actor in our pool
        let actor = {
            let mut actor_pool_map = self.actor_pool_map.lock().await;
            let actor_pool = actor_pool_map
                .entry(script_path.to_string())
                .or_insert(vec![]);

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

    fn create_actor(&self, script_path: &str, script: &str) -> Result<DenoActor<C, M>> {
        DenoActor::new(
            UserCode::LoadFromMemory {
                path: script_path.to_owned(),
                script: script.to_owned(),
            },
            self.config.user_agent_name,
            self.config.shims.clone(),
            self.config.additional_code.clone(),
            self.config.create_extensions,
            self.config.explicit_error_class_name,
            self.config.shared_state.clone(),
            self.config.process_call_context,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generic::deno_module::{Arg, DenoModuleSharedState};
    use serde_json::Value;

    use futures::future::join_all;

    #[tokio::test]
    async fn test_actor_executor() {
        let module_path = "test_js/direct.js";
        let module_script = include_str!("test_js/direct.js");

        let executor_pool = DenoExecutorPool::new(
            "PayasDenoTest",
            vec![],
            vec![],
            None,
            Vec::new,
            |_, _| {},
            DenoModuleSharedState::default(),
        );

        let executor = executor_pool
            .get_executor(module_path, module_script)
            .await
            .unwrap();
        let res = executor
            .execute(
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
        let module_path = "test_js/direct.js";
        let module_script = include_str!("test_js/direct.js");

        let executor_pool = DenoExecutorPool::new(
            "PayasDenoTest",
            vec![],
            vec![],
            None,
            Vec::new,
            |_, _| {},
            DenoModuleSharedState::default(),
        );

        let total_futures = 10;

        let mut handles = vec![];

        async fn execute_function(
            pool: &DenoExecutorPool<(), ()>,
            script_path: &str,
            script: &str,
            method_name: &str,
            arguments: Vec<Arg>,
        ) -> Result<Value> {
            let executor = pool.get_executor(script_path, script).await;
            executor?.execute(method_name, arguments, (), ()).await
        }

        for _ in 1..=total_futures {
            let handle = execute_function(
                &executor_pool,
                module_path,
                module_script,
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
