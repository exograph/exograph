// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use futures::pin_mut;

use crate::error::DenoError;

use super::{deno_actor::DenoActor, deno_module::Arg};
use async_trait::async_trait;
use serde_json::Value;
use std::fmt::Debug;

/// `DenoExecutor` provides a way to execute a method.
///
/// # Implementation
/// It sets up a Tokio channel for the `DenoActor` to use in order to talk back to `DenoExecutor`.
/// Afterwards, it will kick off the execution by awaiting on the `DenoActor`'s asynchronous `execute` method.
/// It will concurrently listen and handle requests from DenoActor sent through the channel by calling the
/// `callback_processor` to resolve callbacks and responding with the final result.
pub struct DenoExecutor<C, M, R> {
    pub(crate) actor: DenoActor<C, M, R>,
}

#[async_trait]
pub trait CallbackProcessor<M> {
    async fn process_callback(&self, req: M);
}

#[async_trait]
impl CallbackProcessor<()> for () {
    async fn process_callback(&self, _req: ()) {}
}

impl<C: Sync + Send + Debug + 'static, M: Sync + Send + 'static, R: Debug + Sync + Send + 'static>
    DenoExecutor<C, M, R>
{
    pub(super) async fn execute(
        &self,
        method_name: &str,
        arguments: Vec<Arg>,
        call_context: C,
        callback_processor: impl CallbackProcessor<M>,
    ) -> Result<(Value, Option<R>), DenoError> {
        // set up a channel for Deno to talk to use through
        let (to_user_sender, mut to_user_receiver) = tokio::sync::mpsc::channel(1);

        // construct a future for our final result
        let on_function_result = self.actor.execute(
            method_name.to_string(),
            arguments,
            call_context,
            to_user_sender,
        );

        pin_mut!(on_function_result); // needs to be pinned to reuse it

        // receive loop
        loop {
            let on_recv_request = to_user_receiver.recv();
            pin_mut!(on_recv_request);

            tokio::select! {
                msg = on_recv_request => {
                    // handle requests from Deno for data
                    callback_processor.process_callback(msg.expect("Channel was dropped before operation completion")).await;
                }

                final_result = &mut on_function_result => {
                    // function has resolved with the return value
                    break final_result;
                },
            }
        }
    }
}
