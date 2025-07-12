// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use deno_core::Extension;
use futures::pin_mut;
use serde_json::Value;
use std::fmt::Debug;
use std::{
    panic,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::{
    Mutex,
    mpsc::{Receiver, Sender},
    oneshot,
};
use tracing::instrument;

use crate::deno_module::{Arg, DenoModule, UserCode};
use crate::error::{DenoError, DenoInternalError};

struct DenoCall<C, R> {
    method_name: String,
    arguments: Vec<Arg>,
    call_context: C,
    /// The sender to communicate the final result
    final_response_sender: oneshot::Sender<Result<(Value, Option<R>), DenoError>>,
}

/// An actor-like wrapper for `DenoModule`.
///
/// The purpose of DenoActor is to isolate DenoModule in its own thread and to provide methods to
/// interact with DenoModule through message passing.
///
/// This behaves like an actor in that it processes only one message at a time. However, it does
/// carry additional mechanism to co-ordinate dealing with intermediate computations.
///
/// # Creation setup:
/// - Set up two channels: one to communicate requests to execute calls and other to communicate
///   callbacks.
/// - Create a thread (each actor has its own thread).
/// - The thread:
///     - Creates a DenoModule instance
///     - Puts the sender of the channel to communicate callback into `DenoModule`'s `op_state` so
///       that shims can access it to send callback messages (see exograph_ops.rs).
///     - Waits for a request to execute call (sent by the `execute` method; see below) and forwards
///       it to the `DenoModule` instance.
///     - Sends the result to the sender of the request.
///
/// # Message flow:
/// - A DenoActor may be asked to execute a JavaScript function by executing the `execute` method
///   passing it the function name, the arguments, an opaque "call context" as well as the sender
///   half of a channel to communicate callbacks (`callback_sender`).
/// - The `execute` method creates a channel to communicate the final result of the call, assembles
///   a `DenoCall` structure (consisting of the function name, arguments, and the sender half of a
///   channel to communicate callbacks). It then send the `DenoCall` structure to `call_sender`
///   (which is created per actor during its creation).
/// - It then loops waiting for receiving a callback message or the final result. If it receives a
///   callback message, it forwards that to the `callback_sender` and loops again. If it receives
///   the final result, it breaks the loop returning that result.
///
/// # Type Parameters
/// * `C` - The type of the call context. Call context is any value (such as the name of the current
///   operation) that the message processing may need
/// * `M` - The type of the message that the actor will receive.
/// * `R` - An opaque, optional, out-of-band type to also return with the method result. This type is
///   extracted from Deno's GothamStorage. Useful for returning information from #[op] blocks.
pub(crate) struct DenoActor<C, M, R> {
    // Receiver to poll for callback messages such as `proceed` or `executeQuery`.
    callback_receiver: Arc<Mutex<Receiver<M>>>,
    // Sender to ask the actor to execute a JS/TS call. The actor will poll for messages on the corresponding receiver.
    call_sender: Sender<DenoCall<C, R>>,
    busy: Arc<std::sync::atomic::AtomicBool>,
}

impl<C, M, R> DenoActor<C, M, R>
where
    C: Sync + Send + std::fmt::Debug + 'static, // Call context
    M: Sync + Send + 'static,                   // Message from Deno
    R: Debug + Sync + Send + 'static,           // OOB Return value
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        code: UserCode,
        shims: Vec<(&'static str, &'static [&'static str])>,
        additional_code: Vec<&'static str>,
        extension_ops: fn() -> Vec<Extension>,
        explicit_error_class_name: Option<&'static str>,
        process_call_context: fn(&mut DenoModule, C) -> (),
    ) -> Result<DenoActor<C, M, R>, DenoError> {
        let (callback_sender, callback_receiver) = tokio::sync::mpsc::channel(1);

        // we will receive DenoCall messages through this channel from call_method
        let (deno_call_sender, mut deno_call_receiver) = tokio::sync::mpsc::channel(1);
        let busy = Arc::new(AtomicBool::new(false));

        let busy_clone = busy.clone();

        // start the DenoModule thread
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
                let deno_module = DenoModule::new(
                    code,
                    shims,
                    additional_code,
                    extension_ops(),
                    explicit_error_class_name,
                    None,
                    None,
                )
                .await;

                let mut deno_module = match deno_module {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("{e:?}");
                        panic!("Could not create new DenoModule in DenoActor thread")
                    }
                };

                // store the request sender in Deno OpState for use by ops
                deno_module
                    .put(callback_sender)
                    .unwrap_or_else(|_| panic!("Could not store request sender in DenoModule"));

                // start a receive loop
                loop {
                    // yield and wait for a DenoCall message
                    let DenoCall {
                        method_name,
                        arguments,
                        call_context,
                        final_response_sender,
                    } = match deno_call_receiver.recv().await {
                        Some(call_info) => call_info,
                        None => break,
                    };

                    busy_clone.store(true, Ordering::Relaxed); // mark DenoActor as busy
                    let _: Option<R> = deno_module.take().expect("take() should not have failed"); // clear any existing R from GothamStorage

                    process_call_context(&mut deno_module, call_context);

                    // execute function
                    let result = deno_module.execute_function(&method_name, arguments).await;

                    // take R from GothamStorage
                    let r: Option<R> = deno_module.take().expect("take() should not have failed");

                    // send result of the Deno function back to call_method
                    final_response_sender
                        .send(result.map(|result| (result, r)))
                        .expect("Could not send result in DenoActor thread");

                    busy_clone.store(false, Ordering::Relaxed); // unmark DenoActor as busy
                }
            });
        });

        Ok(DenoActor {
            callback_receiver: Arc::new(Mutex::new(callback_receiver)),
            call_sender: deno_call_sender,
            busy,
        })
    }

    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::Relaxed)
    }

    /// Call a deno method
    ///
    /// During the invocation there may be callbacks (such as `execute` a query or `proceed` form an interceptor). Those calls
    /// will be relayed to the `callback_sender` sender.
    ///
    /// # Arguments
    /// * `method_name` - the name of the method to call (this must be one of the exported methods in the `code` supplied to `DenoActor::new`)
    /// * `arguments` - the arguments to pass to the method
    /// * `call_context` - opaque call context
    /// * `callback_sender` - the sender to send request for intermediate steps (such as proceed() when performing an around interceptor)
    ///
    #[instrument(
        name = "deno_actor::call_method"
        skip(self, callback_sender)
        )]
    pub async fn execute(
        &self,
        method_name: String,
        arguments: Vec<Arg>,
        call_context: C,
        callback_sender: tokio::sync::mpsc::Sender<M>,
    ) -> Result<(Value, Option<R>), DenoError> {
        // Channel to communicate the final result
        let (final_response_sender, final_result_receiver) = oneshot::channel();

        let deno_call = DenoCall {
            method_name,
            arguments,
            call_context,
            final_response_sender,
        };
        // send it to the DenoModule thread
        self.call_sender.send(deno_call).await.map_err(|err| {
            DenoInternalError::Channel(format!(
                "Could not send method call request to DenoActor thread: {err}"
            ))
        })?;

        pin_mut!(final_result_receiver);

        // receive loop
        loop {
            let mut receiver = self.callback_receiver.lock().await;
            let on_recv_request = receiver.recv();
            pin_mut!(on_recv_request);

            // wait on an event from either a Deno op (callback) or from DenoActor containing the final result of the function
            tokio::select! {
                message = on_recv_request => {
                    // forward callback message from Deno to the caller through the channel they gave us
                    callback_sender.send(
                        message.ok_or_else(|| DenoInternalError::Channel("Channel was dropped before completion while calling method".into()))?
                    ).await.map_err(|err| DenoInternalError::Channel(format!("Could not send request result to DenoActor in call_method ({err})")))?;
                }

                final_result = &mut final_result_receiver => {
                    // final result is received, break the loop with the result
                    break final_result.map_err(|err| DenoInternalError::Channel(format!("Could not receive result from DenoActor thread ({err})")))?;
                }
            };
        }
    }
}

// Need to manually implement `Clone` due to https://github.com/rust-lang/rust/issues/26925
impl<C, M, R> Clone for DenoActor<C, M, R> {
    fn clone(&self) -> Self {
        DenoActor {
            callback_receiver: self.callback_receiver.clone(),
            call_sender: self.call_sender.clone(),
            busy: self.busy.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tokio::sync::mpsc::channel;

    const ADDITIONAL_CODE: &str = "";
    const EXPLICIT_ERROR_CLASS_NAME: Option<&str> = None;

    #[tokio::test]
    async fn test_actor() {
        let actor: DenoActor<(), (), ()> = DenoActor::new(
            UserCode::LoadFromFs(Path::new("src/test_js/direct.js").to_path_buf()),
            vec![],
            vec![ADDITIONAL_CODE],
            Vec::new,
            EXPLICIT_ERROR_CLASS_NAME,
            |_, _| {},
        )
        .unwrap();

        let (to_user_sender, _to_user_receiver) = channel(1);

        let (res, _) = actor
            .execute(
                "addAndDouble".to_string(),
                vec![Arg::Serde(2_i32.into()), Arg::Serde(3_i32.into())],
                (),
                to_user_sender,
            )
            .await
            .unwrap();

        assert_eq!(res, 10);
    }
}
