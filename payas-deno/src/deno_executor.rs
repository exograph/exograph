use std::{collections::HashMap, sync::Arc};

use deno_core::Extension;
use tokio::sync::Mutex;

use futures::pin_mut;

use crate::{
    deno_actor::{
        DenoActor, FnClaytipExecuteQuery, FnClaytipInterceptorProceed, RequestFromDenoMessage,
        ResponseForDenoMessage,
    },
    module::deno_module::{Arg, DenoModuleSharedState, UserCode},
};
use anyhow::Result;
use serde_json::Value;

type DenoActorPoolMap = HashMap<String, DenoActorPool>;
type DenoActorPool = Vec<DenoActor>;

/// DenoExecutor maintains a pool of DenoActors for each module to delegate work to.
///
/// Calling execute_function_with_shims will either select a free actor or allocate a new DenoActor to run the function on.
/// DenoExecutor will then set up a Tokio channel for the DenoActor to use in order to talk back to DenoExecutor.
/// Afterwards, it will kick off the execution by awaiting on the DenoActor's asynchronous `call_method` method.
/// It will concurrently listen and handle requests from DenoActor sent through the channel by calling the
/// appropriate function pointer passed to execute_function_with_shims and responding with the result.
///
/// The hierarchy of modules:
///
/// DenoExecutor -> DenoActor -> DenoModule
///              -> DenoActor -> DenoModule
///              -> DenoActor -> DenoModule
///               ...
#[derive(Default)]
pub struct DenoExecutor {
    actor_pool_map: Arc<Mutex<DenoActorPoolMap>>,
    shared_state: DenoModuleSharedState,
}

fn create_extensions() -> Vec<Extension> {
    // we provide a set of Claytip functionality through custom Deno ops,
    // create a Deno extension that provides these ops
    let ext = Extension::builder()
        .ops(vec![
            crate::claytip_ops::op_claytip_execute_query::decl(),
            crate::claytip_ops::op_intercepted_operation_name::decl(),
            crate::claytip_ops::op_intercepted_proceed::decl(),
        ])
        .build();
    vec![ext]
}

impl<'a> DenoExecutor {
    const SHIMS: [(&'static str, &'static str); 2] = [
        ("ClaytipInjected", include_str!("claytip_shim.js")),
        ("Operation", include_str!("operation_shim.js")),
    ];

    const USER_AGENT: &'static str = "Claytip";
    const ADDITIONAL_CODE: &'static [&'static str] = &[include_str!("./claytip_error.js")];
    const EXPLICIT_ERROR_CLASS_NAME: Option<&'static str> = Some("ClaytipError");

    fn create_actor(&self, script_path: &str, script: &str) -> Result<DenoActor> {
        DenoActor::new(
            UserCode::LoadFromMemory {
                path: script_path.to_owned(),
                script: script.to_owned(),
            },
            Self::USER_AGENT,
            &Self::SHIMS,
            Self::ADDITIONAL_CODE,
            create_extensions,
            Self::EXPLICIT_ERROR_CLASS_NAME,
            self.shared_state.clone(),
        )
    }

    /// Allocate a number of instances for a module.
    pub async fn preload_module(
        &self,
        script_path: &str,
        script: &str,
        instances: usize,
    ) -> Result<()> {
        {
            if let Some(actor_pool) = self.actor_pool_map.lock().await.get(script_path) {
                if actor_pool.len() >= instances {
                    // already have enough instances
                    return Ok(());
                }
            }
        }

        let mut initial_actor_pool = vec![];

        for _ in 0..instances {
            let actor = self.create_actor(script_path, script)?;
            initial_actor_pool.push(actor);
        }

        self.actor_pool_map
            .lock()
            .await
            .insert(script_path.to_owned(), initial_actor_pool);

        Ok(())
    }

    pub async fn execute_function(
        &self,
        script_path: &str,
        script: &str,
        method_name: &str,
        arguments: Vec<Arg>,
    ) -> Result<Value> {
        self.execute_function_with_shims(
            script_path,
            script,
            method_name,
            arguments,
            None,
            None,
            None,
        )
        .await
    }

    // TODO: look at passing a fn pointer struct as an argument
    #[allow(clippy::too_many_arguments)]
    pub async fn execute_function_with_shims(
        &'a self,
        script_path: &str,
        script: &str,
        method_name: &'a str,
        arguments: Vec<Arg>,

        claytip_execute_query: Option<&'a FnClaytipExecuteQuery<'a>>,
        claytip_intercepted_operation_name: Option<String>,
        claytip_proceed: Option<&'a FnClaytipInterceptorProceed<'a>>,
    ) -> Result<Value> {
        // find or allocate a free actor in our pool
        let mut actor = {
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

        // set up a channel for Deno to talk to use through
        let (to_user_sender, mut to_user_receiver) = tokio::sync::mpsc::channel(1);

        // construct a future for our final result
        let on_function_result = actor.call_method(
            method_name.to_string(),
            arguments,
            claytip_intercepted_operation_name,
            to_user_sender,
        );

        pin_mut!(on_function_result); // needs to be pinned to reuse it

        // receive loop
        loop {
            let on_recv_request = to_user_receiver.recv();
            pin_mut!(on_recv_request);

            tokio::select! {
                msg = on_recv_request => {
                    // handle requests from Deno for data
                    match msg.expect("Channel was dropped before operation completion") {
                        RequestFromDenoMessage::InterceptedOperationProceed {
                            response_sender
                        } => {
                            let proceed_result = claytip_proceed.unwrap()().await;
                            response_sender.send(ResponseForDenoMessage::InterceptedOperationProceed(proceed_result)).ok().unwrap();
                        },
                        RequestFromDenoMessage::ClaytipExecute { query_string, variables, response_sender } => {
                            let query_result = claytip_execute_query.unwrap()(query_string, variables).await;
                            response_sender.send(ResponseForDenoMessage::ClaytipExecute(query_result)).ok().unwrap();
                        },
                    }
                }

                final_result = &mut on_function_result => {
                    // function has resolved with the return value
                    break final_result;
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::future::join_all;

    #[tokio::test]
    async fn test_actor_executor() {
        let executor = DenoExecutor::default();

        let module_path = "test_js/direct.js";
        let module_script = include_str!("test_js/direct.js");

        executor
            .preload_module(module_path, module_script, 1)
            .await
            .unwrap();

        let res = executor
            .execute_function(
                module_path,
                module_script,
                "addAndDouble",
                vec![Arg::Serde(2.into()), Arg::Serde(3.into())],
            )
            .await;

        assert_eq!(res.unwrap(), 10);
    }

    #[tokio::test]
    async fn test_actor_executor_concurrent() {
        let executor = DenoExecutor::default();
        let module_path = "test_js/direct.js";
        let module_script = include_str!("test_js/direct.js");
        let total_futures = 10;

        // start with one preloaded DenoModule
        executor
            .preload_module(module_path, module_script, 1)
            .await
            .unwrap();

        let mut handles = vec![];

        for _ in 1..=total_futures {
            let handle = executor.execute_function(
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
