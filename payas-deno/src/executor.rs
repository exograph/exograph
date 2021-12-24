use std::{
    cell::RefCell,
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use actix::{Actor, Addr};
use async_channel::unbounded;
use futures::{pin_mut, select, Future, FutureExt, StreamExt};
use tokio::runtime::Runtime;

use crate::actor::{InProgress, MethodCall};
use crate::{
    actor::{
        FnClaytipExecuteQuery, FnClaytipInterceptorGetName, FnClaytipInterceptorProceed,
        FromDenoMessage, ToDenoMessage,
    },
    Arg, DenoActor, DenoModuleSharedState,
};
use anyhow::Result;
use serde_json::Value;

type DenoActorPool = Vec<Addr<DenoActor>>;

pub struct DenoExecutor {
    actor_pool_map: Arc<Mutex<RefCell<HashMap<PathBuf, DenoActorPool>>>>,
    shared_state: DenoModuleSharedState,
    runtime: Arc<Mutex<Runtime>>,
}

impl<'a> DenoExecutor {
    pub fn new() -> DenoExecutor {
        DenoExecutor {
            actor_pool_map: Default::default(),
            shared_state: Default::default(),
            runtime: Arc::new(Mutex::new(Runtime::new().unwrap())),
        }
    }

    pub async fn preload_module(&self, path: &Path, instances: usize) {
        let actor_pool_map = self.actor_pool_map.lock().unwrap();
        let mut actor_pool_map = actor_pool_map.borrow_mut();

        if let Some(actor_pool) = actor_pool_map.get(path) {
            if actor_pool.len() >= instances {
                // already enough instances
                return;
            }
        }

        let mut initial_actor_pool = vec![];

        for _ in 0..instances {
            let path = path.to_owned();
            let addr = DenoActor::new(&path, self.shared_state.clone())
                .await
                .start();
            initial_actor_pool.push(addr);
        }

        actor_pool_map.insert(path.to_owned(), initial_actor_pool);
    }
    
    pub async fn execute_function(
        &self,
        module_path: &Path,
        method_name: &str,
        arguments: Vec<Arg>,
    ) -> Result<Value> {
        self.execute_function_with_shims(module_path, method_name, arguments, None, None, None).await
    }

    pub fn execute_function_with_shims(
        &'a self,
        module_path: &'a Path,
        method_name: &'a str,
        arguments: Vec<Arg>,

        claytip_execute_query: Option<&'a FnClaytipExecuteQuery>,
        claytip_get_interceptor: Option<&'a FnClaytipInterceptorGetName>,
        claytip_proceed: Option<&'a FnClaytipInterceptorProceed>,
    ) -> impl Future<Output = Result<Value>> + 'a {
        async move {
            println!("inside execution");

            let actor_pool_map = self.actor_pool_map.lock().unwrap();
            let mut actor_pool = actor_pool_map.borrow().get(module_path).unwrap().clone();

            let actor: Addr<DenoActor> = {
                let free_actors: Vec<Addr<DenoActor>> = futures::stream::iter(actor_pool.iter())
                    .filter_map(|addr| async move { // TODO: find map

                        println!("about to await");
                        let is_in_progress = addr.send(InProgress).await;
                        is_in_progress.ok().map(|_| addr.to_owned())
                    })
                    .collect()
                    .await;

                println!("after free_actors");

                if let Some(actor) = free_actors.iter().next() {
                    println!("one free");
                    (*actor).clone()
                } else {
                    println!("none free");

                    // allocate new DenoActor
                    let module_path = module_path.to_owned();
                    actor_pool.push(
                        DenoActor::new(&module_path, self.shared_state.clone())
                            .await
                            .start(),
                    );
                    actor_pool.iter().last().unwrap().clone()
                }
            };

            let (from_user_sender, from_user_receiver) = unbounded();
            let (to_user_sender, to_user_receiver) = unbounded();

            let on_finished = actor
                .send(MethodCall {
                    method_name: method_name.to_string(),
                    arguments,
                    from_user: from_user_receiver,
                    to_user: to_user_sender,
                })
                .fuse();

            let on_recv = to_user_receiver.recv().fuse();
            pin_mut!(on_finished, on_recv);

            println!("looping...");
            loop {
                select! {
                    final_result = on_finished => {

                        println!("finished");
                        break final_result.unwrap();
                    },

                    msg = on_recv => {
                        println!("recv");
                        match msg.unwrap() {
                            FromDenoMessage::RequestInteceptedOperationName => {
                                let name = claytip_get_interceptor.unwrap()();
                                from_user_sender.send(ToDenoMessage::ResponseInteceptedOperationName(name)).await.unwrap();
                            },
                            FromDenoMessage::RequestInteceptedOperationProceed => {
                                let proceed_result = claytip_proceed.unwrap()();
                                from_user_sender.send(ToDenoMessage::ResponseInteceptedOperationProceed(proceed_result)).await.unwrap();
                            },
                            FromDenoMessage::RequestClaytipExecute { query_string, variables } => {
                                let query_result = claytip_execute_query.unwrap()(query_string, variables);
                                from_user_sender.send(ToDenoMessage::ResponseClaytipExecute(query_result)).await.unwrap();
                            },
                        }
                    }
                }
            }
        }
    }
}
