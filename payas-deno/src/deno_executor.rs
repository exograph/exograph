use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use futures::pin_mut;

use crate::{
    deno_actor::{
        FnClaytipExecuteQuery, FnClaytipInterceptorProceed, RequestFromDenoMessage,
        ResponseForDenoMessage,
    },
    Arg, DenoActor, DenoModuleSharedState, UserCode,
};
use anyhow::Result;
use serde_json::Value;

type DenoActorPoolMap = HashMap<String, DenoActorPool>;
type DenoActorPool = Vec<Arc<Mutex<DenoActor>>>;

/// DenoExecutor maintains a pool of DenoActors for each module to delegate work to.
///
/// Calling execute_function_with_shims will either select a free actor or allocate a new DenoActor to run the function on.
/// DenoExecutor will then set up a Tokio channel for the DenoActor to use in order to talk back to DenoExecutor.
/// Afterwards, it will kick off the execution by awaiting on the DenoActor's asynchronous `call_method` method.
/// It will concurrently listen and handle requests from DenoActor sent through the channel by calling the
/// appropriate function pointer passed to execute_function_with_shims and responding with the result.
///
/// The hiearchy of modules:
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

// FIXME: deno cannot be shared across multiple threads, remove unsafe impl Send and .worker(1) in payas-server/lib.rs when following issues are resolved
// https://github.com/denoland/rusty_v8/issues/486 (issue we're seeing)
// https://github.com/denoland/rusty_v8/issues/643
// https://github.com/denoland/rusty_v8/pull/738
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl Send for DenoActor {}

impl<'a> DenoExecutor {
    /// Allocate a number of instances for a module.
    pub async fn preload_module(
        &self,
        script_path: &str,
        script: &str,
        instances: usize,
    ) -> Result<()> {
        let mut actor_pool_map = self.actor_pool_map.lock().unwrap();

        if let Some(actor_pool) = actor_pool_map.get(script_path) {
            if actor_pool.len() >= instances {
                // already have enough instances
                return Ok(());
            }
        }

        let mut initial_actor_pool = vec![];

        for _ in 0..instances {
            let actor = DenoActor::new(
                UserCode::LoadFromMemory {
                    path: script_path.to_owned(),
                    script: script.to_owned(),
                },
                self.shared_state.clone(),
            )
            .await?;
            initial_actor_pool.push(Arc::new(Mutex::new(actor)));
        }

        actor_pool_map.insert(script_path.to_owned(), initial_actor_pool);

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
        // grab a copy of the actor pool for module_path
        let actor_pool_copy = {
            let mut actor_pool_map = self
                .actor_pool_map
                .lock()
                .expect("Poisoned actor pool mutex in Deno executor")
                .clone();
            let actor_pool = actor_pool_map
                .entry(script_path.to_string())
                .or_insert_with(Vec::new);

            actor_pool.clone()
        };

        #[allow(unused_assignments)]
        let mut actor_mutex: Option<Arc<Mutex<DenoActor>>> = None;

        // try to acquire a lock on an actor from our pool
        let try_lock = actor_pool_copy.iter().find_map(|addr| addr.try_lock().ok());
        let mut actor = if let Some(actor) = try_lock {
            // found a free actor!
            actor
        } else {
            // no free actors; need to allocate a new DenoActor
            let module_path = script_path.to_owned();
            let new_actor = DenoActor::new(
                UserCode::LoadFromMemory {
                    path: script_path.to_owned(),
                    script: script.to_string(),
                },
                self.shared_state.clone(),
            )
            .await?;
            actor_mutex = Some(Arc::new(Mutex::new(new_actor)));

            {
                // add new actor to the pool
                let mut actor_pool_map = self
                    .actor_pool_map
                    .lock()
                    .expect("Poisoned actor pool mutex in Deno executor")
                    .clone();

                let actor_pool = actor_pool_map.get_mut(&module_path).unwrap();
                actor_pool.push(actor_mutex.clone().unwrap());
            }

            // acquire a lock from our new mutex
            actor_mutex
                .as_deref()
                .unwrap()
                .lock()
                .expect("Poisoned actor mutex in Deno executor")
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
