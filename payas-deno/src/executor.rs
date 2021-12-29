use std::{
    cell::RefCell,
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};

use futures::{pin_mut, select, Future, FutureExt, StreamExt};

use crate::actor::MethodCall;
use crate::{
    actor::{
        FnClaytipExecuteQuery, FnClaytipInterceptorGetName, FnClaytipInterceptorProceed,
        FromDenoMessage, ToDenoMessage,
    },
    Arg, DenoActor, DenoModuleSharedState,
};
use anyhow::Result;
use serde_json::Value;

type DenoActorPoolMap = HashMap<PathBuf, DenoActorPool>;
type DenoActorPool = Vec<Arc<Mutex<DenoActor>>>;

pub struct DenoExecutor {
    actor_pool_map: Arc<Mutex<DenoActorPoolMap>>,
    shared_state: DenoModuleSharedState,
}

unsafe impl Send for DenoActor {}

impl<'a> DenoExecutor {
    pub fn new() -> DenoExecutor {
        DenoExecutor {
            actor_pool_map: Default::default(),
            shared_state: Default::default(),
        }
    }

    pub async fn preload_module(&self, path: &Path, instances: usize) {
        let mut actor_pool_map = self.actor_pool_map.lock().unwrap();

        if let Some(actor_pool) = actor_pool_map.get(path) {
            if actor_pool.len() >= instances {
                // already enough instances
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
        println!("locking actor pool map...");

        let actor_pool_copy = {
            let mut actor_pool_map = self.actor_pool_map.try_lock().unwrap().clone();
            let actor_pool = actor_pool_map
                .entry(module_path.to_path_buf())
                .or_insert(vec![]);

            actor_pool.clone()
        };

        let mut actor_mutex: Option<Arc<Mutex<DenoActor>>> = None;

        let lock =
            actor_pool_copy
                .iter()
                .find_map(|addr| addr.try_lock().ok());

        let mut actor = if let Some(actor) = lock {
            println!("one free");
            actor
        } else {
            println!("none free, allocating");

            // allocate new DenoActor
            let module_path = module_path.to_owned();
            let new_actor = DenoActor::new(&module_path, self.shared_state.clone()).await;
            actor_mutex = Some(Arc::new(Mutex::new(new_actor)));

            {
                let mut actor_pool_map = self.actor_pool_map.lock().unwrap().clone();
                let actor_pool = actor_pool_map.get_mut(&module_path).unwrap();
                actor_pool.push(actor_mutex.clone().unwrap());
            }

            actor_mutex.as_deref().unwrap().lock().unwrap()
        };

        let (to_user_sender, mut to_user_receiver) = tokio::sync::mpsc::channel(1);

        let on_finished = actor
            .handle(MethodCall {
                method_name: method_name.to_string(),
                arguments,
                to_user: to_user_sender,
            });

        pin_mut!(on_finished);

        loop {
            println!("executor recv loop turn start");
            let on_recv = to_user_receiver.recv();
            pin_mut!(on_recv);

            tokio::select! {
                msg = on_recv => {
                    match msg.unwrap() {
                        FromDenoMessage::RequestInteceptedOperationName {
                            response_sender
                        } => {
                            println!("executor recv loop: name request!");
                            let name = claytip_get_interceptor.unwrap()();
                            response_sender.send(ToDenoMessage::ResponseInteceptedOperationName(name)).ok().unwrap();
                        },
                        FromDenoMessage::RequestInteceptedOperationProceed {
                            response_sender
                        } => {
                            println!("executor recv loop: proceed request!");
                            let proceed_result = claytip_proceed.unwrap()().await;
                            response_sender.send(ToDenoMessage::ResponseInteceptedOperationProceed(proceed_result)).ok().unwrap();
                        },
                        FromDenoMessage::RequestClaytipExecute { query_string, variables, response_sender } => {
                            println!("executor recv loop: execution request!");
                            let query_result = claytip_execute_query.unwrap()(query_string, variables).await;
                            response_sender.send(ToDenoMessage::ResponseClaytipExecute(query_result)).ok().unwrap();
                        },
                    }
                }

                final_result = &mut on_finished => {
                    println!("executor recv loop: got final result");
                    break final_result;
                },
            }
        }
    }
}
