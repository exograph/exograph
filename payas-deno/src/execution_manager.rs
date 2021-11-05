use std::{
    collections::HashMap,
    panic,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use anyhow::Result;
use crossbeam_channel::{unbounded, Receiver, Sender};
use deno_runtime::tokio_util;
use serde_json::Value;

use crate::{deno_module::DenoModuleSharedState, Arg, DenoModule};

lazy_static::lazy_static! {
    static ref TRANSACTION_ID_COUNTER: Arc<Mutex<u64>> = {
        Arc::new(
            Mutex::new(
                0
            )
        )
    };
}

pub struct DenoExecutionManager {
    to_thread: Sender<ExecutionTransaction<ToThreadMessage>>,
    from_thread: Receiver<ExecutionTransaction<FromThreadMessage>>,
}

enum ToThreadMessage {
    RequestInvokeMethod(PathBuf, String, Vec<Arg>),
    ResponseClaytipExecute(Result<Value>),
    ResponseInteceptedOperationName(String),
    ResponseInteceptedOperationProceed(Result<Value>), // This should be Result<QueryResponse>, but we don't have that in the scope
}

enum FromThreadMessage {
    RequestInteceptedOperationName,
    RequestInteceptedOperationProceed,
    RequestClaytipExecute {
        query_string: String,
        variables: Option<serde_json::Map<String, Value>>,
    },
    ResponseInvokeMethod(Result<Value>),
}

struct ExecutionTransaction<T> {
    transaction_id: u64,
    message: T,
}

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

impl Default for DenoExecutionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DenoExecutionManager {
    pub fn new() -> Self {
        let (to_thread, from_user) = unbounded();
        let (to_user, from_thread) = unbounded();

        std::thread::spawn(move || Self::execution_thread(from_user, to_user));

        DenoExecutionManager {
            to_thread,
            from_thread,
        }
    }

    fn execution_thread(
        from_user: Receiver<ExecutionTransaction<ToThreadMessage>>,
        to_user: Sender<ExecutionTransaction<FromThreadMessage>>,
    ) {
        let shared_state = DenoModuleSharedState::default(); // set of shared resources and connecting channels between Deno modules
        let mut transactions_map: HashMap<u64, Sender<ToThreadMessage>> = HashMap::new();

        // recv loop, read in transaction messages
        loop {
            let ExecutionTransaction {
                transaction_id,
                message,
            } = from_user.recv().unwrap();
            // utility closure for responding to a transaction
            let respond_to_user = {
                let to_user = to_user.clone();

                move |msg: FromThreadMessage| {
                    to_user
                        .send(ExecutionTransaction {
                            transaction_id,
                            message: msg,
                        })
                        .unwrap();
                }
            };

            // do we have a transaction in progress for this transaction ID?
            if let Some(to_deno) = transactions_map.get(&transaction_id) {
                // transaction in progress, forward message to deno
                to_deno.send(message).unwrap()
            } else {
                // no such transaction, are we starting one?
                if let ToThreadMessage::RequestInvokeMethod(module_path, method_name, arguments) =
                    message
                {
                    // start transaction

                    // create channels to communicate between the Deno thread and this one
                    let (to_deno, to_deno_endpoint) = unbounded();
                    let (from_deno, from_deno_endpoint) = unbounded();

                    // store transaction with a Sender that will send messages to the Deno thread
                    transactions_map.insert(transaction_id, to_deno);

                    let shared_state = shared_state.clone();

                    // start a thread to drain & forward messages from Deno to the user
                    // kill thread when we receive ResponseInvokeMethod, the last valid message that
                    // will be sent from the Deno thread
                    std::thread::spawn(move || loop {
                        match from_deno_endpoint.recv().unwrap() {
                            msg @ FromThreadMessage::ResponseInvokeMethod(_) => {
                                respond_to_user(msg);
                                break;
                            }
                            msg => respond_to_user(msg),
                        }
                    });

                    // start Deno thread
                    std::thread::spawn(move || {
                        let runtime = tokio_util::create_basic_runtime();
                        let from_deno = from_deno;

                        // create a Deno module
                        // TODO: can we cache the initial state of each module to avoid the startup cost?
                        let mut deno_module =
                            futures::executor::block_on(Self::create_deno_module(
                                module_path,
                                from_deno.clone(),
                                to_deno_endpoint,
                                shared_state,
                            ));

                        deno_module.preload_function(vec![&method_name]);

                        let res =
                            runtime.block_on(deno_module.execute_function(&method_name, arguments));
                        from_deno
                            .send(FromThreadMessage::ResponseInvokeMethod(res))
                            .unwrap();
                    });
                } else {
                    // only valid message that can start a transaction is RequestInvokeMethod
                    panic!()
                }
            }
        }
    }

    async fn create_deno_module(
        path: PathBuf,
        to_user: Sender<FromThreadMessage>,
        from_user: Receiver<ToThreadMessage>,
        shared_state: DenoModuleSharedState,
    ) -> DenoModule {
        let shims = vec![
            ("ClaytipInjected", include_str!("claytip_shim.js")),
            ("Operation", include_str!("operation_shim.js")),
        ];

        DenoModule::new(
            &path,
            "Claytip",
            &shims,
            &move |runtime| {
                let mut sync_ops = vec![];

                add_op!(
                    sync_ops,
                    "op_claytip_execute_query",
                    to_user,
                    from_user,
                    move |args: Vec<String>,
                          sender: Sender<FromThreadMessage>,
                          receiver: Receiver<ToThreadMessage>| {
                        let query_string = &args[0];
                        let variables: Option<serde_json::Map<String, Value>> =
                            args.get(1).map(|vars| serde_json::from_str(vars).unwrap());

                        sender
                            .send(FromThreadMessage::RequestClaytipExecute {
                                query_string: query_string.to_owned(),
                                variables,
                            })
                            .unwrap();

                        if let ToThreadMessage::ResponseClaytipExecute(result) =
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
                    to_user,
                    from_user,
                    |_: (),
                     sender: Sender<FromThreadMessage>,
                     receiver: Receiver<ToThreadMessage>| {
                        sender
                            .send(FromThreadMessage::RequestInteceptedOperationName)
                            .unwrap();

                        if let ToThreadMessage::ResponseInteceptedOperationName(result) =
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
                    to_user,
                    from_user,
                    |_: (),
                     sender: Sender<FromThreadMessage>,
                     receiver: Receiver<ToThreadMessage>| {
                        sender
                            .send(FromThreadMessage::RequestInteceptedOperationProceed)
                            .unwrap();

                        if let ToThreadMessage::ResponseInteceptedOperationProceed(result) =
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
        .unwrap()
    }

    pub fn execute_function(
        &self,
        module_path: &Path,
        method_name: &str,
        args: Vec<Arg>,
        // TODO: this should become a context struct?
        // TODO: could thes arguments be removed (and moved as constructor args)?
        execute_query: &dyn Fn(String, Option<&serde_json::Map<String, Value>>) -> Result<Value>,
        get_intercepted_operation_name: Option<&dyn Fn() -> String>,
        proceed_intercepted_operation: Option<&dyn Fn() -> Result<Value>>,
    ) -> Result<serde_json::Value> {
        // grab a transaction id
        let transaction_id = {
            let mut counter = TRANSACTION_ID_COUNTER.lock().unwrap();
            *counter += 1;
            *counter
        };

        println!(
            "Executing {} at {}",
            method_name,
            module_path.to_string_lossy()
        );

        let respond_to_execution_thread = |msg: ToThreadMessage| {
            self.to_thread
                .send(ExecutionTransaction {
                    transaction_id,
                    message: msg,
                })
                .unwrap();
        };

        respond_to_execution_thread(ToThreadMessage::RequestInvokeMethod(
            module_path.into(),
            method_name.into(),
            args,
        ));

        let mut from_iter = self.from_thread.iter().peekable();

        loop {
            let message = from_iter.peek().unwrap();

            if message.transaction_id == transaction_id {
                let consumed_message = from_iter.next().unwrap();

                match consumed_message.message {
                    FromThreadMessage::RequestInteceptedOperationName => {
                        let operation_name = get_intercepted_operation_name.unwrap()();
                        respond_to_execution_thread(
                            ToThreadMessage::ResponseInteceptedOperationName(operation_name),
                        );
                    }
                    FromThreadMessage::RequestInteceptedOperationProceed => {
                        let res = proceed_intercepted_operation.unwrap()();
                        respond_to_execution_thread(
                            ToThreadMessage::ResponseInteceptedOperationProceed(res),
                        );
                    }
                    FromThreadMessage::RequestClaytipExecute {
                        query_string,
                        variables,
                    } => {
                        let result = execute_query(query_string, variables.as_ref());
                        respond_to_execution_thread(ToThreadMessage::ResponseClaytipExecute(
                            result,
                        ));
                    }
                    FromThreadMessage::ResponseInvokeMethod(result) => return result,
                }
            }
        }
    }
}
