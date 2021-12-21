use std::panic;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use actix::Context;
use actix::{Actor, Handler, Message};
use anyhow::Result;
use futures::stream::StreamExt;
use futures::{pin_mut, select, FutureExt};
use serde_json::Value;

use crossbeam_channel::{unbounded, Receiver, Sender};

use crate::{Arg, DenoModule, DenoModuleSharedState};

pub struct DenoActor {
    deno_module: DenoModule,
    in_progress: AtomicBool,
    deno_sender: Sender<ToDenoMessage>,
    deno_receiver: Receiver<FromDenoMessage>,
}

pub enum FromDenoMessage {
    RequestInteceptedOperationName,
    RequestInteceptedOperationProceed,
    RequestClaytipExecute {
        query_string: String,
        variables: Option<serde_json::Map<String, Value>>,
    },
}

pub enum ToDenoMessage {
    ResponseInteceptedOperationName(String),
    ResponseInteceptedOperationProceed(Result<Value>),
    ResponseClaytipExecute(Result<Value>),
}

pub type FnClaytipExecuteQuery<'a> =
    (dyn Fn(String, Option<serde_json::Map<String, Value>>) -> Result<Value> + 'a);
pub type FnClaytipInterceptorGetName<'a> = (dyn Fn() -> String + 'a);
pub type FnClaytipInterceptorProceed<'a> = (dyn Fn() -> Result<Value> + 'a);

macro_rules! add_op {
    ($sync_ops:expr, $name:expr, $sender:expr, $receiver:expr, $op:expr) => {
        let sender = $sender.clone();
        let receiver = $receiver.clone();
        $sync_ops.push((
            $name,
            deno_core::op_sync(move |_state, args, _: ()| {
                $op(args, sender.clone(), receiver.clone())
            }),
        ))
    };
}

impl DenoActor {
    pub async fn new(path: &Path, shared_state: DenoModuleSharedState) -> DenoActor {
        let shims = vec![
            ("ClaytipInjected", include_str!("claytip_shim.js")),
            ("Operation", include_str!("operation_shim.js")),
        ];

        // TODO
        let (from_deno_sender, from_deno_receiver) = unbounded();
        let (to_deno_sender, to_deno_receiver) = unbounded();

        let deno_module = DenoModule::new(
            &path,
            "Claytip",
            &shims,
            &move |runtime| {
                let mut sync_ops = vec![];

                add_op!(
                    sync_ops,
                    "op_claytip_execute_query",
                    from_deno_sender,
                    to_deno_receiver,
                    move |args: Vec<String>,
                          sender: Sender<FromDenoMessage>,
                          receiver: Receiver<ToDenoMessage>| {
                        let query_string = &args[0];
                        let variables: Option<serde_json::Map<String, Value>> =
                            args.get(1).map(|vars| serde_json::from_str(vars).unwrap());

                        sender
                            .send(FromDenoMessage::RequestClaytipExecute {
                                query_string: query_string.to_owned(),
                                variables,
                            })
                            .unwrap();

                        if let ToDenoMessage::ResponseClaytipExecute(result) =
                            receiver.recv().unwrap()
                        {
                            result
                        } else {
                            panic!()
                        }
                    }
                );

                add_op!(
                    sync_ops,
                    "op_intercepted_operation_name",
                    from_deno_sender,
                    to_deno_receiver,
                    |_: (), sender: Sender<FromDenoMessage>, receiver: Receiver<ToDenoMessage>| {
                        sender
                            .send(FromDenoMessage::RequestInteceptedOperationName)
                            .unwrap();

                        if let ToDenoMessage::ResponseInteceptedOperationName(result) =
                            receiver.recv().unwrap()
                        {
                            Ok(result)
                        } else {
                            panic!()
                        }
                    }
                );

                add_op!(
                    sync_ops,
                    "op_intercepted_proceed",
                    from_deno_sender,
                    to_deno_receiver,
                    |_: (), sender: Sender<FromDenoMessage>, receiver: Receiver<ToDenoMessage>| {
                        sender
                            .send(FromDenoMessage::RequestInteceptedOperationProceed)
                            .unwrap();

                        if let ToDenoMessage::ResponseInteceptedOperationProceed(result) =
                            receiver.recv().unwrap()
                        {
                            result
                        } else {
                            panic!()
                        }
                    }
                );

                for (name, op) in sync_ops {
                    runtime.register_op(name, op);
                }
            },
            shared_state,
        )
        .await
        .unwrap();

        DenoActor {
            deno_module,
            deno_receiver: from_deno_receiver,
            deno_sender: to_deno_sender,
            in_progress: AtomicBool::new(false),
        }
    }

    pub fn in_progress(&self) -> bool {
        self.in_progress.load(Ordering::Relaxed)
    }
}

impl Actor for DenoActor {
    type Context = Context<Self>;
}

#[derive(Message)]
#[rtype(result = "Result<Value>")]
pub struct MethodCall {
    pub method_name: String,
    pub arguments: Vec<Arg>,

    pub to_user: Sender<FromDenoMessage>,
    pub from_user: Receiver<ToDenoMessage>,
}

unsafe impl Send for MethodCall {}

impl Handler<MethodCall> for DenoActor {
    type Result = Result<Value>;

    fn handle(&mut self, msg: MethodCall, _: &mut Self::Context) -> Self::Result {
        println!("Executing {}", &msg.method_name,);

        self.in_progress.store(true, Ordering::Relaxed);

        // load function by name in module
        self.deno_module.preload_function(vec![&msg.method_name]);

        let future = async {
            let finished = self
                .deno_module
                .execute_function(&msg.method_name, msg.arguments)
                .fuse();

            let mut recv_stream = futures::stream::iter(self.deno_receiver.iter());
            let recv = recv_stream.next().fuse();

            pin_mut!(finished, recv);

            loop {
                //println!("loop");
                select! {
                    final_result = finished => {
                        break final_result;
                    },

                    message = recv => {
                        msg.to_user.send(message.unwrap()).unwrap();
                        let result = msg.from_user.recv().unwrap();
                        self.deno_sender.send(result).unwrap();
                    },
                };
            }
        };

        let val = futures::executor::block_on(future);
        self.in_progress.store(false, Ordering::Relaxed);
        val
    }
}

#[derive(Message)]
#[rtype(result = "bool")]
pub struct InProgress;

impl Handler<InProgress> for DenoActor {
    type Result = bool;

    fn handle(&mut self, _: InProgress, _: &mut Self::Context) -> Self::Result {
        self.in_progress()
    }
}
