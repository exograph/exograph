use std::sync::atomic::Ordering;
use std::{
    collections::HashMap,
    panic,
    path::{Path, PathBuf},
    sync::{atomic::AtomicUsize, Arc, Mutex},
};

use anyhow::Result;
use crossbeam_channel::{unbounded, Receiver, Sender};
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::{deno_module::DenoModuleSharedState, Arg, DenoModule};

static TRANSACTION_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub struct DenoExecutionManager {
    to_manager_thread: Sender<ExecutionTransaction<ToDenoMessage>>,
    from_manager_thread: Receiver<ExecutionTransaction<FromDenoMessage>>,
}

enum ToDenoMessage {
    StartMethod(PathBuf, String, Vec<Arg>),
    ResponseClaytipExecute(Result<Value>),
    ResponseInteceptedOperationName(String),
    ResponseInteceptedOperationProceed(Result<Value>), // This should be Result<QueryResponse>, but we don't have that in the scope
}

enum FromDenoMessage {
    RequestInteceptedOperationName,
    RequestInteceptedOperationProceed,
    RequestClaytipExecute {
        query_string: String,
        variables: Option<serde_json::Map<String, Value>>,
    },
    EndMethod(Result<Value>),
}

struct ExecutionTransaction<T> {
    transaction_id: usize,
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

// What happens in DenoExecutionManager:
//
// 1. When DenoExecutionManager::new() is called, we start a 'manager thread' and a pair of channels
//    to communicate with the thread.
//
//    The manager thread's responsibility is to spawn threads for DenoModules to live
//    in (as they are not Sync nor Send), as well as forward messages to and from users through impl methods
//    on DenoExecutionManager.
//
// 2. A user invokes a method call with a specific module by calling execute_function with the appropriate
//    arguments. This function initiates a 'transaction' by sending a ToDenoMessage::StartMethod wrapped in
//    a ExecutionTransaction to the manager thread with a new ID.
//
// 3. The manager thread looks up the received message by ID in its transaction map. Finding none (as it is a
//    new transaction), it creates a pair of channels and two threads using tokio:
//       a. a DenoModule thread that initializes a DenoModule and invokes the actual DenoModule::execute_function()
//       b. a thread that drains messages from the DenoModule thread, wraps it with the correct transaction ID, and sends it
//          to the user.
//
// 4. The requested function runs in the DenoModule thread (from step 3a). DenoExecutionManager::execution_function now blocks sits in a recv loop,
//    waiting for messages from the thread with the  in step 3b.
//
//    Figure below depicts the flow of ExecutionTransaction<...> messages.
//   __________________________________________                                              __________________________
//   |                                        | <--            drain thread (3b)         <-- |                        |
//   | DenoExecutionManager::execute_function |                                              | DenoModule thread (3a) |
//   |________________________________________| --> DenoExecutionManager::manager_thread --> |________________________|
//
// 5. The DenoModule may invoke a shim, causing the DenoExecutionManager::execute_function loop
//    to receive a ExecutionTransaction<FromDenoMessage> that is not EndMethod.
//
//    In this case, DenoExecutionManager::execute_function will execute the relevant operation closure and send back a
//    message with the result.
//
// 6. The DenoModule will, at some point, resolve the transaction by sending a FromDenoMessage::EndMethod with the result of the
//    method call. The transaction is removed from the manager thread's transaction map and both the 3a and 3b should die along
//    with its channels; the transaction is considered finished at this point.
//
// 7. DenoExecutionManager::execute_function returns the result of the method call.
//
impl DenoExecutionManager {
    pub fn new() -> Self {
        let (to_manager_thread, from_user) = unbounded();
        let (to_user, from_manager_thread) = unbounded();

        std::thread::spawn(move || Self::manager_thread(from_user, to_user));

        DenoExecutionManager {
            to_manager_thread,
            from_manager_thread,
        }
    }

    fn manager_thread(
        from_user: Receiver<ExecutionTransaction<ToDenoMessage>>,
        to_user: Sender<ExecutionTransaction<FromDenoMessage>>,
    ) {
        let shared_state = DenoModuleSharedState::default(); // set of shared resources and connecting channels between Deno modules
        let transactions_map: Arc<Mutex<HashMap<usize, Sender<ToDenoMessage>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let tokio_runtime = Runtime::new().unwrap();

        // recv loop, read in transaction messages
        loop {
            let ExecutionTransaction {
                transaction_id,
                message,
            } = from_user.recv().unwrap();
            // utility closure for responding to a transaction
            let send_to_user = {
                let to_user = to_user.clone();

                move |msg: FromDenoMessage| {
                    to_user
                        .send(ExecutionTransaction {
                            transaction_id,
                            message: msg,
                        })
                        .unwrap();
                }
            };

            let maybe_transaction = {
                let transactions_map = transactions_map.lock().unwrap();
                transactions_map.get(&transaction_id).cloned()
            };

            // do we have a transaction in progress for this transaction ID?
            if let Some(to_deno) = maybe_transaction {
                // transaction in progress, forward message to deno
                to_deno.send(message).unwrap()
            } else {
                // no such transaction, are we starting one?
                if let ToDenoMessage::StartMethod(module_path, method_name, arguments) = message {
                    // start transaction

                    // create channels to communicate between the Deno thread and this one
                    let (to_deno, to_deno_endpoint) = unbounded();
                    let (from_deno, from_deno_endpoint) = unbounded();

                    // store transaction with a Sender that will send messages to the Deno thread
                    {
                        let mut transactions_map = transactions_map.lock().unwrap();
                        transactions_map.insert(transaction_id, to_deno);
                    }

                    // clone shared Arc<>s for new DenoModule
                    let shared_state = shared_state.clone();

                    // start Deno thread
                    tokio_runtime.spawn_blocking(move || {
                        let from_deno = from_deno;

                        // create a Deno module
                        // TODO: can we cache the initial state of each module to avoid the startup cost?
                        let mut deno_module =
                            futures::executor::block_on(Self::create_deno_module(
                                module_path,
                                shared_state,
                                from_deno.clone(),
                                to_deno_endpoint,
                            ));

                        // load function by name in module
                        deno_module.preload_function(vec![&method_name]);

                        let res = futures::executor::block_on(
                            deno_module.execute_function(&method_name, arguments),
                        );
                        from_deno.send(FromDenoMessage::EndMethod(res)).unwrap();
                    });

                    // start a thread to drain & forward messages from Deno to the user
                    // kill thread when we receive ResponseInvokeMethod, the last valid message that
                    // will be sent from the Deno thread
                    {
                        let transaction_map = transactions_map.clone();
                        tokio_runtime.spawn_blocking(move || loop {
                            match from_deno_endpoint.recv().unwrap() {
                                msg @ FromDenoMessage::EndMethod(_) => {
                                    // remove transaction from transaction_map
                                    let mut transaction_map = transaction_map.lock().unwrap();
                                    transaction_map.remove(&transaction_id);

                                    // wrap message in ExecutionTransaction and send to user
                                    send_to_user(msg);
                                    break;
                                }

                                // wrap message in ExecutionTransaction and send to user
                                msg => send_to_user(msg),
                            }
                        });
                    }
                } else {
                    // only valid message that can start a transaction is RequestInvokeMethod
                    panic!()
                }
            }
        }
    }

    /// Helper for DenoModule creation.
    ///
    /// In addition to module-related parameters, the function takes a Sender and Receiver used to
    /// complete shim operations related to query execution and interceptors.
    async fn create_deno_module(
        path: PathBuf,
        shared_state: DenoModuleSharedState,
        to_user: Sender<FromDenoMessage>,
        from_user: Receiver<ToDenoMessage>,
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
                    to_user,
                    from_user,
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
                    to_user,
                    from_user,
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
        let transaction_id = TRANSACTION_ID_COUNTER.fetch_add(1, Ordering::Relaxed);

        println!(
            "Executing {} at {}",
            method_name,
            module_path.to_string_lossy()
        );

        let send_to_manager_thread = |msg: ToDenoMessage| {
            self.to_manager_thread
                .send(ExecutionTransaction {
                    transaction_id,
                    message: msg,
                })
                .unwrap();
        };

        // initiate a method transaction with our new transaction id
        send_to_manager_thread(ToDenoMessage::StartMethod(
            module_path.into(),
            method_name.into(),
            args,
        ));

        let mut from_iter = self.from_manager_thread.iter().peekable();

        // listen and respond to messages from the execution thread until it
        // returns EndMethod
        loop {
            // recv loop, blocks until we get a message from the execution thread
            let message = from_iter.peek().unwrap();

            if message.transaction_id == transaction_id {
                let consumed_message = from_iter.next().unwrap();

                match consumed_message.message {
                    FromDenoMessage::RequestInteceptedOperationName => {
                        let operation_name = get_intercepted_operation_name.unwrap()();
                        send_to_manager_thread(ToDenoMessage::ResponseInteceptedOperationName(
                            operation_name,
                        ));
                    }
                    FromDenoMessage::RequestInteceptedOperationProceed => {
                        let res = proceed_intercepted_operation.unwrap()();
                        send_to_manager_thread(ToDenoMessage::ResponseInteceptedOperationProceed(
                            res,
                        ));
                    }
                    FromDenoMessage::RequestClaytipExecute {
                        query_string,
                        variables,
                    } => {
                        let result = execute_query(query_string, variables.as_ref());
                        send_to_manager_thread(ToDenoMessage::ResponseClaytipExecute(result));
                    }

                    // return method result
                    FromDenoMessage::EndMethod(result) => return result,
                }
            }
        }
    }
}
