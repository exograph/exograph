use std::{collections::HashMap, sync::Arc};

use deno_core::Extension;
use tokio::sync::{oneshot, Mutex};

use futures::pin_mut;

use crate::{
    deno_actor::DenoActor,
    module::deno_module::{Arg, DenoModuleSharedState, UserCode},
    DenoModule,
};
use anyhow::Result;
use async_trait::async_trait;
use futures::future::BoxFuture;
use serde_json::Value;

type DenoActorPoolMap = HashMap<String, DenoActorPool>;
type DenoActorPool = Vec<DenoActor<Option<String>, RequestFromDenoMessage>>;

pub enum RequestFromDenoMessage {
    InterceptedOperationProceed {
        response_sender: oneshot::Sender<ResponseForDenoMessage>,
    },
    ClaytipExecute {
        query_string: String,
        variables: Option<serde_json::Map<String, Value>>,
        response_sender: oneshot::Sender<ResponseForDenoMessage>,
    },
}

pub enum ResponseForDenoMessage {
    InterceptedOperationProceed(Result<Value>),
    ClaytipExecute(Result<Value>),
}

pub type FnClaytipExecuteQuery<'a> = (dyn Fn(String, Option<serde_json::Map<String, Value>>) -> BoxFuture<'a, Result<Value>>
     + 'a
     + Send
     + Sync);
pub type FnClaytipInterceptorProceed<'a> =
    (dyn Fn() -> BoxFuture<'a, Result<Value>> + 'a + Send + Sync);

pub fn process_call_context(deno_module: &mut DenoModule, call_context: Option<String>) {
    deno_module
        .put(crate::claytip_ops::InterceptedOperationName(call_context))
        .unwrap_or_else(|_| panic!("Failed to setup interceptor"));
}

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
pub struct DenoExecutor {
    actor: DenoActor<Option<String>, RequestFromDenoMessage>,
}

#[async_trait]
pub trait CallbackProcessor {
    async fn process_callback(&self, req: RequestFromDenoMessage);
}
pub struct ClayCallbackProcessor<'a> {
    pub claytip_execute_query: Option<&'a FnClaytipExecuteQuery<'a>>,
    pub claytip_proceed: Option<&'a FnClaytipInterceptorProceed<'a>>,
}

#[async_trait]
impl<'a> CallbackProcessor for ClayCallbackProcessor<'a> {
    async fn process_callback(&self, req: RequestFromDenoMessage) {
        match req {
            RequestFromDenoMessage::InterceptedOperationProceed { response_sender } => {
                let proceed_result = self.claytip_proceed.unwrap()().await;
                response_sender
                    .send(ResponseForDenoMessage::InterceptedOperationProceed(
                        proceed_result,
                    ))
                    .ok()
                    .unwrap();
            }
            RequestFromDenoMessage::ClaytipExecute {
                query_string,
                variables,
                response_sender,
            } => {
                let query_result =
                    self.claytip_execute_query.unwrap()(query_string, variables).await;
                response_sender
                    .send(ResponseForDenoMessage::ClaytipExecute(query_result))
                    .ok()
                    .unwrap();
            }
        }
    }
}

#[async_trait]
impl CallbackProcessor for () {
    async fn process_callback(&self, _req: RequestFromDenoMessage) {}
}

impl<'a> DenoExecutor {
    pub async fn execute(
        &self,
        method_name: &str,
        arguments: Vec<Arg>,
        call_context: Option<String>,
        callback_processor: impl CallbackProcessor,
    ) -> Result<Value> {
        // set up a channel for Deno to talk to use through
        let (to_user_sender, mut to_user_receiver) = tokio::sync::mpsc::channel(1);

        // construct a future for our final result
        let on_function_result = self.actor.execute(
            method_name.to_string(),
            arguments,
            call_context,
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
                    callback_processor. process_callback(msg.expect("Channel was dropped before operation completion")).await;
                }

                final_result = &mut on_function_result => {
                    // function has resolved with the return value
                    break final_result;
                },
            }
        }
    }
}

pub struct DenoExecutorConfig {
    user_agent_name: &'static str,
    shims: Vec<(&'static str, &'static str)>,
    additional_code: Vec<&'static str>,
    explicit_error_class_name: Option<&'static str>,
    create_extensions: fn() -> Vec<Extension>,
    shared_state: DenoModuleSharedState,
}

pub struct DenoExecutorPool {
    config: DenoExecutorConfig,

    actor_pool_map: Arc<Mutex<DenoActorPoolMap>>,
}

const SHIMS: [(&str, &str); 2] = [
    ("ClaytipInjected", include_str!("claytip_shim.js")),
    ("Operation", include_str!("operation_shim.js")),
];

const USER_AGENT: &str = "Claytip";
const ADDITIONAL_CODE: &[&str] = &[include_str!("./claytip_error.js")];
const EXPLICIT_ERROR_CLASS_NAME: Option<&'static str> = Some("ClaytipError");

impl DenoExecutorPool {
    pub fn new(config: DenoExecutorConfig) -> Self {
        Self {
            config,
            actor_pool_map: Arc::new(Mutex::new(DenoActorPoolMap::default())),
        }
    }

    pub fn clay_config() -> DenoExecutorConfig {
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

        DenoExecutorConfig {
            user_agent_name: USER_AGENT,
            shims: SHIMS.to_vec(),
            additional_code: ADDITIONAL_CODE.to_vec(),
            explicit_error_class_name: EXPLICIT_ERROR_CLASS_NAME,
            create_extensions,
            shared_state: DenoModuleSharedState::default(),
        }
    }

    // TODO: look at passing a fn pointer struct as an argument
    #[allow(clippy::too_many_arguments)]
    pub async fn get_executor(&self, script_path: &str, script: &str) -> Result<DenoExecutor> {
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

    fn create_actor(
        &self,
        script_path: &str,
        script: &str,
    ) -> Result<DenoActor<Option<String>, RequestFromDenoMessage>> {
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
            process_call_context,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::future::join_all;

    #[tokio::test]
    async fn test_actor_executor() {
        let module_path = "test_js/direct.js";
        let module_script = include_str!("test_js/direct.js");

        let executor_pool = DenoExecutorPool::new(DenoExecutorConfig {
            user_agent_name: "Claytip_Test",
            shims: vec![],
            additional_code: vec![],
            explicit_error_class_name: None,
            create_extensions: Vec::new,
            shared_state: DenoModuleSharedState::default(),
        });

        let executor = executor_pool
            .get_executor(module_path, module_script)
            .await
            .unwrap();
        let res = executor
            .execute(
                "addAndDouble",
                vec![Arg::Serde(2.into()), Arg::Serde(3.into())],
                None,
                (),
            )
            .await;

        assert_eq!(res.unwrap(), 10);
    }

    #[tokio::test]
    async fn test_actor_executor_concurrent() {
        let module_path = "test_js/direct.js";
        let module_script = include_str!("test_js/direct.js");

        let executor_pool = DenoExecutorPool::new(DenoExecutorConfig {
            user_agent_name: "Claytip_Test",
            shims: vec![],
            additional_code: vec![],
            explicit_error_class_name: None,
            create_extensions: Vec::new,
            shared_state: DenoModuleSharedState::default(),
        });

        let total_futures = 10;

        let mut handles = vec![];

        async fn execute_function(
            pool: &DenoExecutorPool,
            script_path: &str,
            script: &str,
            method_name: &str,
            arguments: Vec<Arg>,
        ) -> Result<Value> {
            let executor = pool.get_executor(script_path, script).await;
            executor?.execute(method_name, arguments, None, ()).await
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
