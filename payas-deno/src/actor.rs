use anyhow::{anyhow, Result};
use deno_core::JsRuntime;
use futures::future::LocalBoxFuture;
use futures::pin_mut;
use serde_json::Value;
use std::panic;
use std::path::Path;
use tokio::sync::mpsc::Receiver;

use crate::{Arg, DenoModule, DenoModuleSharedState};

/// An actor-like wrapper for DenoModule.
pub struct DenoActor {
    deno_module: DenoModule,
    from_deno_receiver: Receiver<RequestFromDenoMessage>,
}

pub enum RequestFromDenoMessage {
    InteceptedOperationProceed {
        response_sender: tokio::sync::oneshot::Sender<ResponseForDenoMessage>,
    },
    ClaytipExecute {
        query_string: String,
        variables: Option<serde_json::Map<String, Value>>,
        response_sender: tokio::sync::oneshot::Sender<ResponseForDenoMessage>,
    },
}

pub enum ResponseForDenoMessage {
    InteceptedOperationProceed(Result<Value>),
    ClaytipExecute(Result<Value>),
}

struct InterceptedOperationName(Option<String>);

pub type FnClaytipExecuteQuery<'a> = (dyn Fn(String, Option<serde_json::Map<String, Value>>) -> LocalBoxFuture<'a, Result<Value>>
     + 'a);
pub type FnClaytipInterceptorProceed<'a> = (dyn Fn() -> LocalBoxFuture<'a, Result<Value>> + 'a);

pub struct MethodCall {
    pub method_name: String,
    pub arguments: Vec<Arg>,

    pub claytip_intercepted_operation_name: Option<String>,

    pub to_user: tokio::sync::mpsc::Sender<RequestFromDenoMessage>,
}

impl DenoActor {
    pub async fn new(path: &Path, shared_state: DenoModuleSharedState) -> DenoActor {
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
                        let mut sender = from_deno_sender.clone();

                        async move {
                            let query_string = &args[0];
                            let variables: Option<serde_json::Map<String, Value>> =
                                args.get(1).map(|vars| serde_json::from_str(vars).unwrap());

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
                                .unwrap();

                            if let ResponseForDenoMessage::ClaytipExecute(result) =
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
                ops.push((
                    "op_intercepted_operation_name",
                    deno_core::op_sync(move |state, _: (), (): _| {
                        // try to read the intercepted operation name out of Deno's GothamStorage
                        if let InterceptedOperationName(Some(name)) = state.try_borrow().unwrap() {
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
                        let mut sender = from_deno_sender.clone();

                        async move {
                            let (response_sender, response_receiver) =
                                tokio::sync::oneshot::channel();

                            sender
                                .send(RequestFromDenoMessage::InteceptedOperationProceed {
                                    response_sender,
                                })
                                .await
                                .ok()
                                .unwrap();

                            if let ResponseForDenoMessage::InteceptedOperationProceed(result) =
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

            for (name, op) in ops {
                runtime.register_op(name, op);
            }
        };

        let deno_module = DenoModule::new(path, "Claytip", &shims, register_ops, shared_state);

        let deno_module = deno_module.await.unwrap();

        DenoActor {
            deno_module,
            from_deno_receiver,
        }
    }

    pub async fn handle(&mut self, mut msg: MethodCall) -> Result<Value> {
        println!("Executing {}", &msg.method_name);

        // put the intercepted operation name into Deno's op_state
        self.deno_module.put(InterceptedOperationName(
            msg.claytip_intercepted_operation_name,
        ));

        let finished = self
            .deno_module
            .execute_function(&msg.method_name, msg.arguments);

        pin_mut!(finished);

        loop {
            let recv = self.from_deno_receiver.recv();
            pin_mut!(recv);

            tokio::select! {
                message = recv => {
                    // forward message from Deno to the caller through the channel they gave us
                    msg.to_user.send(message.unwrap()).await.ok().unwrap();
                }

                final_result = &mut finished => {
                    // clear the intercepted operation name from Deno's op_state

                    break final_result;
                }
            };
        }
    }
}
