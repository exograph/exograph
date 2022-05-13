use anyhow::{anyhow, Result};
use deno_core::Extension;
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

use crate::module::deno_module::{Arg, DenoModule, DenoModuleSharedState, UserCode};

/// An actor-like wrapper for DenoModule.
#[derive(Clone)]
pub(crate) struct DenoActor<C> {
    deno_requests_receiver: Arc<Mutex<Receiver<RequestFromDenoMessage>>>,
    deno_call_sender: Sender<(String, Vec<Arg>, C, oneshot::Sender<DenoResult>)>,
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

type DenoResult = Result<Value>;

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
///
/// JavaScript code running on Deno can invoke preregistered Rust code through Deno.core.op_sync() or Deno.core.op_async().
/// We use Deno ops to facilitate operations such as executing Claytip queries directly from JavaScript.
/// Deno ops cannot be re-registered or unregistered; ops must stay static, which presents a problem if we want to
/// dynamically change what the operations do from request to request (like in the case of the proceed()
/// call from @around interceptors).
///
/// To work around this, DenoActor adopts another layer of message passing (separate from the DenoCall and DenoResult messages)
/// to handle operations. On creation, DenoActor will first initialize a Tokio mpsc channel. It will also initialize an instance
/// of DenoModule and register operations that will send a RequestFromDenoMessage to the channel on invocation. This way, the
/// actual operation does not have to change, just the recipient of Deno op request messages.
impl<C> DenoActor<C>
where
    C: Sync + Send + std::fmt::Debug + 'static,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        code: UserCode,
        user_agent_name: &'static str,
        shims: &'static [(&'static str, &'static str)],
        additional_code: &'static [&'static str],
        extension_ops: fn() -> Vec<Extension>,
        explicit_error_class_name: Option<&'static str>,
        shared_state: DenoModuleSharedState,
        process_call_context: fn(&mut DenoModule, C) -> (),
    ) -> Result<DenoActor<C>> {
        let (from_deno_sender, from_deno_receiver) = tokio::sync::mpsc::channel(1);

        // we will receive DenoCall messages through this channel from call_method
        let (tx, mut rx) =
            tokio::sync::mpsc::channel::<(String, Vec<Arg>, C, oneshot::Sender<DenoResult>)>(1);

        let tx_clone = tx.clone();

        // start the DenoModule thread
        let busy = Arc::new(AtomicBool::new(false));
        let busy_clone = busy.clone();
        std::thread::spawn(move || {
            // we use new_current_thread to explicitly select the current thread scheduler for tokio
            // (don't want to spawn more threads on top of this new one if we don't need one)
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Could not start tokio runtime in DenoActor thread");

            // we use a LocalSet here because Deno futures are not Send, and we need them to be
            // executed in the same thread
            let local = tokio::task::LocalSet::new();

            local.block_on(&runtime, async {
                // first, initialize the Deno module
                let mut deno_module = DenoModule::new(
                    code,
                    user_agent_name,
                    shims,
                    additional_code,
                    extension_ops(),
                    shared_state,
                    explicit_error_class_name,
                )
                .await
                .expect("Could not create new DenoModule in DenoActor thread");

                // store the request sender in Deno OpState for use by ops
                deno_module
                    .put(from_deno_sender)
                    .unwrap_or_else(|_| panic!("Could not store request sender in DenoModule"));

                // start a receive loop
                loop {
                    // yield and wait for a DenoCall message
                    let (function_name, function_args, function_context, response_sender) =
                        match rx.recv().await {
                            Some(call_info) => call_info,
                            // check if the channel is closed (happens sometimes during shutdown). If so break, otherwise we end up
                            // printing an error message after the shutdown message
                            None if tx_clone.is_closed() => break,
                            None => panic!("Could not receive requests in DenoActor thread"),
                        };

                    busy_clone.store(true, Ordering::Relaxed); // mark DenoActor as busy

                    process_call_context(&mut deno_module, function_context);

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
        call_context: C,
        to_user_sender: tokio::sync::mpsc::Sender<RequestFromDenoMessage>,
    ) -> Result<Value> {
        // we will receive the final function result through this channel
        let (tx, rx) = oneshot::channel();

        // send it to the DenoModule thread
        self.deno_call_sender
            .send((method_name, arguments, call_context, tx))
            .await
            .map_err(|err| {
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

            // wait on an event from either a Deno op or from DenoActor containing the final result of the function
            tokio::select! {
                message = on_recv_request => {
                    // forward message from Deno to the caller through the channel they gave us
                    to_user_sender.send(
                        message.ok_or_else(|| anyhow!("Channel was dropped before completion while calling method"))?
                    ).await.map_err(|err| anyhow!("Could not send request result to DenoActor in call_method ({})", err))?;
                }

                final_result = &mut on_function_result => {
                    break final_result.map_err(|err| anyhow!("Could not receive result from DenoActor thread ({})", err))?;
                }
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tokio::sync::mpsc::channel;

    const USER_AGENT_NAME: &str = "Claytip";
    const ADDITIONAL_CODE: &str = "";
    const EXPLICIT_ERROR_CLASS_NAME: Option<&str> = None;

    #[tokio::test]
    async fn test_actor() {
        let mut actor = DenoActor::new(
            UserCode::LoadFromFs(Path::new("src/test_js/direct.js").to_path_buf()),
            USER_AGENT_NAME,
            &[],
            &[ADDITIONAL_CODE],
            Vec::new,
            EXPLICIT_ERROR_CLASS_NAME,
            DenoModuleSharedState::default(),
            |_, _| {},
        )
        .unwrap();

        let (to_user_sender, _to_user_receiver) = channel(1);

        let res = actor
            .call_method(
                "addAndDouble".to_string(),
                vec![Arg::Serde(2.into()), Arg::Serde(3.into())],
                (),
                to_user_sender,
            )
            .await;

        assert_eq!(res.unwrap(), 10);
    }
}
