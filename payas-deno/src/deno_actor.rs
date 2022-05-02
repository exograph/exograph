use anyhow::{anyhow, Result};
use deno_core::JsRuntime;
use futures::future::BoxFuture;
use futures::pin_mut;
use serde_json::Value;
use std::{
    panic,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::{
    mpsc::{Receiver, Sender},
    oneshot, Mutex,
};
use tracing::instrument;

use crate::{Arg, DenoModule, DenoModuleSharedState, UserCode};

/// An actor-like wrapper for DenoModule.
#[derive(Clone)]
pub struct DenoActor {
    deno_requests_receiver: Arc<Mutex<Receiver<RequestFromDenoMessage>>>,
    deno_call_sender: Sender<DenoCall>,
    busy: Arc<std::sync::atomic::AtomicBool>,
}

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

pub struct DenoCall {
    function_name: String,
    function_args: Vec<Arg>,
    intercepted_op_name: Option<String>,
    response_sender: oneshot::Sender<DenoResult>,
}

type DenoResult = Result<Value>;

struct InterceptedOperationName(Option<String>);

pub type FnClaytipExecuteQuery<'a> = (dyn Fn(String, Option<serde_json::Map<String, Value>>) -> BoxFuture<'a, Result<Value>>
     + 'a
     + Send
     + Sync);
pub type FnClaytipInterceptorProceed<'a> =
    (dyn Fn() -> BoxFuture<'a, Result<Value>> + 'a + Send + Sync);

/// A wrapper around DenoModule.
///
/// The purpose of DenoActor is to isolate DenoModule in its own thread and to provide methods to interact
/// with DenoModule through message passing.
impl DenoActor {
    pub fn new(code: UserCode, shared_state: DenoModuleSharedState) -> Result<DenoActor> {
        let shims = vec![
            ("ClaytipInjected", include_str!("claytip_shim.js")),
            ("Operation", include_str!("operation_shim.js")),
        ];

        let (from_deno_sender, from_deno_receiver) = tokio::sync::mpsc::channel(1);

        let register_ops = move |runtime: &mut JsRuntime| {
            let mut ops = vec![];

            {
                let from_deno_sender = from_deno_sender.clone();

                ops.push((
                    "op_claytip_execute_query",
                    deno_core::op_async(move |_state, args: Vec<String>, (): _| {
                        let sender = from_deno_sender.clone();

                        async move {
                            let query_string = &args[0];
                            let variables: Option<serde_json::Map<String, Value>> =
                                args.get(1).map(|vars| {
                                    serde_json::from_str(vars)
                                        .expect("Could not un-stringify variables from Deno during op_claytip_execute_query")
                                });

                            let (response_sender, response_receiver) =
                                tokio::sync::oneshot::channel();

                            sender
                                .send(RequestFromDenoMessage::ClaytipExecute {
                                    query_string: query_string.to_owned(),
                                    variables,
                                    response_sender,
                                })
                                .await
                                .ok()
                                .expect("Could not send request from op_claytip_execute_query");

                            if let ResponseForDenoMessage::ClaytipExecute(result) =
                                response_receiver.await.expect("Could not receive result in op_claytip_execute_query")
                            {
                                result
                            } else {
                                panic!()
                            }
                        }
                    }),
                ));
            }

            {
                ops.push((
                    "op_intercepted_operation_name",
                    deno_core::op_sync(move |state, _: (), (): _| {
                        // try to read the intercepted operation name out of Deno's GothamStorage
                        if let InterceptedOperationName(Some(name)) = state.borrow() {
                            Ok(name.clone())
                        } else {
                            Err(anyhow!("no stored operation name"))
                        }
                    }),
                ));
            }

            {
                let from_deno_sender = from_deno_sender.clone();

                ops.push((
                    "op_intercepted_proceed",
                    deno_core::op_async(move |_state, _: (), (): _| {
                        let sender = from_deno_sender.clone();

                        async move {
                            let (response_sender, response_receiver) =
                                tokio::sync::oneshot::channel();

                            sender
                                .send(RequestFromDenoMessage::InterceptedOperationProceed {
                                    response_sender,
                                })
                                .await
                                .ok()
                                .expect("Could not send request from op_intercepted_proceed");

                            if let ResponseForDenoMessage::InterceptedOperationProceed(result) =
                                response_receiver
                                    .await
                                    .expect("Could not receive result in op_intercepted_proceed")
                            {
                                result
                            } else {
                                panic!()
                            }
                        }
                    }),
                ));
            }

            for (name, op) in ops {
                runtime.register_op(name, op);
            }
        };

        // we will receive DenoCall messages through this channel from call_method
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);

        // start the DenoModule thread
        let busy = Arc::new(AtomicBool::new(false));
        let busy_clone = busy.clone();
        std::thread::spawn(move || {
            // we use new_current_thread to explictly select the current thread scheduler for tokio
            // (don't want to spawn more threads on top of this new one if we don't need one)
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .expect("Could not start tokio runtime in DenoActor thread");
            let local = tokio::task::LocalSet::new();

            local.block_on(&runtime, async {
                // first, initialize the Deno module
                let mut deno_module =
                    DenoModule::new(code, "Claytip", &shims, register_ops, shared_state)
                        .await
                        .expect("Could not create new DenoModule in DenoActor thread");

                // start a receive loop
                loop {
                    // yield and wait for a DenoCall message
                    let DenoCall {
                        function_name,
                        function_args,
                        intercepted_op_name,
                        response_sender,
                    } = rx
                        .recv()
                        .await
                        .expect("Could not receive requests in DenoActor thread");
                    busy_clone.store(true, Ordering::Relaxed); // mark DenoActor as busy

                    deno_module.put(InterceptedOperationName(intercepted_op_name)); // store intercepted operation name into Deno's op_state

                    // execute function
                    let result = deno_module
                        .execute_function(&function_name, function_args)
                        .await;

                    // send result of the Deno function back to call_method
                    response_sender
                        .send(result)
                        .expect("Could not send result in DenoActor thread");

                    busy_clone.store(false, Ordering::Relaxed); // unmark DenoActor as busy
                }
            });
        });

        Ok(DenoActor {
            deno_requests_receiver: Arc::new(Mutex::new(from_deno_receiver)),
            deno_call_sender: tx,
            busy,
        })
    }

    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::Relaxed)
    }

    #[instrument(
        name = "deno_actor::call_method"
        skip(self, to_user_sender)
        )]
    pub async fn call_method(
        &mut self,
        method_name: String,
        arguments: Vec<Arg>,
        claytip_intercepted_operation_name: Option<String>,
        to_user_sender: tokio::sync::mpsc::Sender<RequestFromDenoMessage>,
    ) -> Result<Value> {
        // we will receive the final function result through this channel
        let (tx, rx) = oneshot::channel();

        // construct a DenoCall message
        let deno_call = DenoCall {
            function_name: method_name,
            function_args: arguments,
            intercepted_op_name: claytip_intercepted_operation_name,
            response_sender: tx,
        };

        // send it to the DenoModule thread
        self.deno_call_sender.send(deno_call).await.map_err(|err| {
            anyhow!(
                "Could not send method call request to DenoActor thread ({})",
                err
            )
        })?;

        let on_function_result = rx;
        pin_mut!(on_function_result);

        // receive loop
        loop {
            let mut receiver = self.deno_requests_receiver.lock().await;
            let on_recv_request = receiver.recv();
            pin_mut!(on_recv_request);

            tokio::select! {
                message = on_recv_request => {
                    // forward message from Deno to the caller through the channel they gave us
                    to_user_sender.send(
                        message.ok_or(anyhow!("Channel was dropped before completion while calling method"))?
                    ).await.map_err(|err| anyhow!("Could not send request result to DenoActor in call_method ({})", err))?;
                }

                final_result = &mut on_function_result => {
                    break final_result.map_err(|err| anyhow!("Could not receive result from DenoActor thread ({})", err))?;
                }
            };
        }
    }
}
