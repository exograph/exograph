use anyhow::{anyhow, Result};
use deno_core::JsRuntime;
use futures::future::LocalBoxFuture;
use futures::pin_mut;
use serde_json::Value;
use std::panic;
use tokio::sync::mpsc::Receiver;

use crate::{Arg, DenoModule, DenoModuleSharedState, UserCode};

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

/// A wrapper around DenoModule.
///
/// DenoActor exists only to make passing invoking operations from DenoModule easier. JavaScript code running on Deno can
/// invoke preregistered Rust code through Deno.core.op_sync() or Deno.core.op_async(). We use Deno ops to facilitate
/// operations such as executing Claytip queries directly from JavaScript.
///
/// Deno ops cannot be re-registered or unregistered; ops must stay static, which presents a problem if we want to
/// dynamically change what the operations do (like in the case of the proceed() call from @around interceptors).
///
/// To work around this, DenoActor adopts message passing to handle operations. On creation, DenoActor will first
/// initialize a Tokio mpsc channel. It will also initialize an instance of DenoModule and register operations that will send a
/// RequestFromDenoMessage to the channel on invocation. This way, the actual operation does not have to change, just the recipient
/// of Deno op request messages.
///
/// A complete Deno op would consist of an exchange that looks like this:
/// ________________                                                                                                          _______________________________
/// |    caller    | -> DenoActor.call_method -> DenoModule.execute_function -> {user code} --------------------------------> |    Deno.core.opAsync(...)   |      |
/// |              |                                                                                                          |                             |      |
/// |              | <-- to_user_sender  <- DenoActor forwarding loop <- from_deno_sender  <- [ RequestFromDenoMessage ] <--- |                             |     time
/// |              |                                                                                                          |                             |      |
/// |              | -> [ ResponseForDenoMessage ] -> response_sender ------------------------------------------------------> |                             |      V
/// |              |                                                                                                          |                             |
/// |______________|                                                                                           {user code} <- |_____________________________|
///
impl DenoActor {
    pub async fn new(code: UserCode, shared_state: DenoModuleSharedState) -> Result<DenoActor> {
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
                                .expect("Could not send request from op_intercepted_proceed");

                            if let ResponseForDenoMessage::InteceptedOperationProceed(result) =
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

        let deno_module = DenoModule::new(
            code,
            "Claytip",
            &shims,
            register_ops,
            shared_state,
        );

        let deno_module = deno_module.await?;

        Ok(DenoActor {
            deno_module,
            from_deno_receiver,
        })
    }

    pub async fn call_method(
        &mut self,
        method_name: String,
        arguments: Vec<Arg>,
        claytip_intercepted_operation_name: Option<String>,
        mut to_user_sender: tokio::sync::mpsc::Sender<RequestFromDenoMessage>,
    ) -> Result<Value> {
        println!("Executing {}", &method_name);

        // put the intercepted operation name into Deno's op_state
        self.deno_module
            .put(InterceptedOperationName(claytip_intercepted_operation_name));

        let on_function_result = self.deno_module.execute_function(&method_name, arguments);

        pin_mut!(on_function_result);

        loop {
            let on_recv_request = self.from_deno_receiver.recv();
            pin_mut!(on_recv_request);

            tokio::select! {
                message = on_recv_request => {
                    // forward message from Deno to the caller through the channel they gave us
                    to_user_sender.send(
                        message.expect("Channel was dropped before completion while calling method")
                    ).await.ok().expect("Could not send result to Deno in call_method");
                }

                final_result = &mut on_function_result => {
                    break final_result;
                }
            };
        }
    }
}
