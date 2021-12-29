use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use futures::pin_mut;

use crate::actor::MethodCall;
use crate::{
    actor::{
        FnClaytipExecuteQuery, FnClaytipInterceptorGetName, FnClaytipInterceptorProceed,
        RequestFromDenoMessage, ResponseForDenoMessage,
    },
    Arg, DenoActor, DenoModuleSharedState,
};
use anyhow::Result;
use serde_json::Value;

type DenoActorPoolMap = HashMap<PathBuf, DenoActorPool>;
type DenoActorPool = Vec<Arc<Mutex<DenoActor>>>;

/// DenoExecutor maintains a pool of DenoActors for each module to delegate work to.
///
/// Calling execute_function* will either select a free actor or allocate a new DenoActor
/// for the function.
#[derive(Default)]
pub struct DenoExecutor {
    actor_pool_map: Arc<Mutex<DenoActorPoolMap>>,
    shared_state: DenoModuleSharedState,
}

// FIXME: deno cannot be shared across multiple threads, remove unsafe impl Send and .worker(1) in payas-server/lib.rs when following issues are resolved
// https://github.com/denoland/rusty_v8/issues/486 (issue we're seeing)
// https://github.com/denoland/rusty_v8/issues/643
// https://github.com/denoland/rusty_v8/pull/738
unsafe impl Send for DenoActor {}

impl<'a> DenoExecutor {
    /// Allocate a number of instances for a module.
    pub async fn preload_module(&self, path: &Path, instances: usize) {
        let mut actor_pool_map = self.actor_pool_map.lock().unwrap();

        if let Some(actor_pool) = actor_pool_map.get(path) {
            if actor_pool.len() >= instances {
                // already have enough instances
                return;
            }
        }

        let mut initial_actor_pool = vec![];

        for _ in 0..instances {
            let path = path.to_owned();
            let actor = DenoActor::new(&path, self.shared_state.clone()).await;
            initial_actor_pool.push(Arc::new(Mutex::new(actor)));
        }

        actor_pool_map.insert(path.to_owned(), initial_actor_pool);
    }

    pub async fn execute_function(
        &self,
        module_path: &Path,
        method_name: &str,
        arguments: Vec<Arg>,
    ) -> Result<Value> {
        self.execute_function_with_shims(module_path, method_name, arguments, None, None, None)
            .await
    }

    pub async fn execute_function_with_shims(
        &'a self,
        module_path: &'a Path,
        method_name: &'a str,
        arguments: Vec<Arg>,

        claytip_execute_query: Option<&'a FnClaytipExecuteQuery<'a>>,
        claytip_get_interceptor: Option<&'a FnClaytipInterceptorGetName<'a>>,
        claytip_proceed: Option<&'a FnClaytipInterceptorProceed<'a>>,
    ) -> Result<Value> {
        // grab a copy of the actor pool for module_path
        let actor_pool_copy = {
            let mut actor_pool_map = self.actor_pool_map.try_lock().unwrap().clone();
            let actor_pool = actor_pool_map
                .entry(module_path.to_path_buf())
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
            let module_path = module_path.to_owned();
            let new_actor = DenoActor::new(&module_path, self.shared_state.clone()).await;
            actor_mutex = Some(Arc::new(Mutex::new(new_actor)));

            {
                // add new actor to the pool
                let mut actor_pool_map = self.actor_pool_map.lock().unwrap().clone();
                let actor_pool = actor_pool_map.get_mut(&module_path).unwrap();
                actor_pool.push(actor_mutex.clone().unwrap());
            }

            // acquire a lock from our new mutex
            actor_mutex.as_deref().unwrap().lock().unwrap()
        };

        // set up a channel for Deno to talk to use through
        let (to_user_sender, mut to_user_receiver) = tokio::sync::mpsc::channel(1);

        // construct a future for our final result
        let on_finished_future = actor.handle(MethodCall {
            method_name: method_name.to_string(),
            arguments,
            to_user: to_user_sender,
        });

        pin_mut!(on_finished_future); // needs to be pinned to reuse it

        // receive loop
        loop {
            let on_recv = to_user_receiver.recv();
            pin_mut!(on_recv);

            tokio::select! {
                msg = on_recv => {
                    // handle requests from Deno for data
                    match msg.unwrap() {
                        RequestFromDenoMessage::InteceptedOperationName {
                            response_sender
                        } => {
                            let name = claytip_get_interceptor.unwrap()();
                            response_sender.send(ResponseForDenoMessage::InteceptedOperationName(name)).ok().unwrap();
                        },
                        RequestFromDenoMessage::InteceptedOperationProceed {
                            response_sender
                        } => {
                            let proceed_result = claytip_proceed.unwrap()().await;
                            response_sender.send(ResponseForDenoMessage::InteceptedOperationProceed(proceed_result)).ok().unwrap();
                        },
                        RequestFromDenoMessage::ClaytipExecute { query_string, variables, response_sender } => {
                            let query_result = claytip_execute_query.unwrap()(query_string, variables).await;
                            response_sender.send(ResponseForDenoMessage::ClaytipExecute(query_result)).ok().unwrap();
                        },
                    }
                }

                final_result = &mut on_finished_future => {
                    break final_result;
                },
            }
        }
    }
}
