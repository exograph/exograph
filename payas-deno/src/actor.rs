use std::panic;
use std::path::Path;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use deno_core::JsRuntime;
use futures::future::LocalBoxFuture;
use futures::{pin_mut, select, Future, FutureExt};
use serde_json::Value;

use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::{deno_module, Arg, DenoModule, DenoModuleSharedState};

pub struct DenoActor {
    deno_module: DenoModule,
    from_deno_receiver: Receiver<FromDenoMessage>,
}

pub enum FromDenoMessage {
    RequestInteceptedOperationName {
        response_sender: tokio::sync::oneshot::Sender<ToDenoMessage>
    },
    RequestInteceptedOperationProceed {
        response_sender: tokio::sync::oneshot::Sender<ToDenoMessage>
    },
    RequestClaytipExecute {
        query_string: String,
        variables: Option<serde_json::Map<String, Value>>,
        response_sender: tokio::sync::oneshot::Sender<ToDenoMessage>
    },
}

pub enum ToDenoMessage {
    ResponseInteceptedOperationName(String),
    ResponseInteceptedOperationProceed(Result<Value>),
    ResponseClaytipExecute(Result<Value>),
}

pub type FnClaytipExecuteQuery<'a> = (dyn Fn(String, Option<serde_json::Map<String, Value>>) -> LocalBoxFuture<'a, Result<Value>>
     + 'a);
pub type FnClaytipInterceptorGetName<'a> = (dyn Fn() -> String + 'a);
pub type FnClaytipInterceptorProceed<'a> = (dyn Fn() -> LocalBoxFuture<'a, Result<Value>> + 'a);

pub struct MethodCall {
    pub method_name: String,
    pub arguments: Vec<Arg>,

    pub to_user: tokio::sync::mpsc::Sender<FromDenoMessage>,
}

impl DenoActor {
    pub async fn new(path: &Path, shared_state: DenoModuleSharedState) -> DenoActor {
        let shims = vec![
            ("ClaytipInjected", include_str!("claytip_shim.js")),
            ("Operation", include_str!("operation_shim.js")),
        ];

        // TODO
        let (from_deno_sender, from_deno_receiver) = tokio::sync::mpsc::channel(1);

        let register_ops = move |runtime: &mut JsRuntime| {
            let mut async_ops = vec![];

            {
                let from_deno_sender = from_deno_sender.clone();

                async_ops.push((
                    "op_claytip_execute_query",
                    deno_core::op_async(move |_state, args: Vec<String>, (): _| {
                        let mut sender = from_deno_sender.clone();

                        println!("op_claytip_execute_query");
                        async move {
                            println!("op_claytip_execute_query future start");

                            let query_string = &args[0];
                            let variables: Option<serde_json::Map<String, Value>> =
                                args.get(1).map(|vars| serde_json::from_str(vars).unwrap());

                            let (response_sender, response_receiver) = tokio::sync::oneshot::channel();

                            println!("op_claytip_execute_query send...");
                            sender
                                .send(FromDenoMessage::RequestClaytipExecute {
                                    query_string: query_string.to_owned(),
                                    variables,
                                    response_sender
                                })
                                .await
                                .ok()
                                .unwrap();


                            println!("op_claytip_execute_query recv...");
                            //println!("exec2");
                            if let ToDenoMessage::ResponseClaytipExecute(result) =
                                response_receiver.await.unwrap()
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
                let from_deno_sender = from_deno_sender.clone();

                async_ops.push((
                    "op_intercepted_operation_name",
                    deno_core::op_async(move |_state, _: (), (): _| {
                        let mut sender = from_deno_sender.clone();

                        println!("op_intercepted_operation_name");
                        async move {
                            println!("op_intercepted_operation_name future start");
                            let (response_sender, response_receiver) = tokio::sync::oneshot::channel();

                            println!("op_intercepted_operation_name send...");
                            sender
                                .send(FromDenoMessage::RequestInteceptedOperationName {
                                    response_sender
                                })
                                .await
                                .ok()
                                .unwrap();

                            println!("op_intercepted_operation_name recv...");
                            if let ToDenoMessage::ResponseInteceptedOperationName(result) =
                                response_receiver.await.unwrap()
                            {
                                Ok(result)
                            } else {
                                panic!()
                            }
                        }
                    }),
                ));
            }

            {
                let from_deno_sender = from_deno_sender.clone();

                async_ops.push((
                    "op_intercepted_proceed",
                    deno_core::op_async(move |_state, _: (), (): _| {
                        let mut sender = from_deno_sender.clone();

                        println!("op_intercepted_proceed");
                        async move {
                            println!("op_intercepted_proceed future start");

                            let (response_sender, response_receiver) = tokio::sync::oneshot::channel();

                            println!("op_intercepted_proceed send...");
                            sender
                                .send(FromDenoMessage::RequestInteceptedOperationProceed {
                                    response_sender
                                })
                                .await
                                .ok()
                                .unwrap();

                            println!("op_intercepted_proceed recv...");
                            if let ToDenoMessage::ResponseInteceptedOperationProceed(result) =
                                response_receiver.await.unwrap()
                            {
                                result
                            } else {
                                panic!()
                            }
                        }
                    }),
                ));
            }

            for (name, op) in async_ops {
                runtime.register_op(name, op);
            }
        };

        let deno_module = DenoModule::new(&path, "Claytip", &shims, register_ops, shared_state);

        let deno_module = deno_module.await.unwrap();

        DenoActor {
            deno_module,
            from_deno_receiver: from_deno_receiver,
        }
    }

    pub async fn handle(&mut self, mut msg: MethodCall) -> Result<Value> {
        println!("Executing {}", &msg.method_name);

        // load function by name in module
        self.deno_module.preload_function(vec![&msg.method_name]);

        let finished = self
            .deno_module
            .execute_function(&msg.method_name, msg.arguments);

        pin_mut!(finished);

        loop {
            println!("actor recv loop turn start");
            let recv = self.from_deno_receiver.recv();
            pin_mut!(recv);

            tokio::select! {
                message = recv => {
                    println!("actor recv loop: got from_deno_receiver message, forwarding to user");
                    msg.to_user.send(message.unwrap()).await.ok().unwrap();
                }

                final_result = &mut finished => {
                    println!("actor recv loop: got final result");
                    break final_result;
                }
            };
        }
    }
}
