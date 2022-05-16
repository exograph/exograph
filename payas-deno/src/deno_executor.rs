use futures::pin_mut;

use crate::{deno_actor::DenoActor, module::deno_module::Arg};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

/// DenoExecutor maintains a pool of DenoActors for each module to delegate work to.
///
/// Calling execute_function_with_shims will either select a free actor or allocate a new DenoActor to run the function on.
/// DenoExecutor will then set up a Tokio channel for the DenoActor to use in order to talk back to DenoExecutor.
/// Afterwards, it will kick off the execution by awaiting on the DenoActor's asynchronous `call_method` method.
/// It will concurrently listen and handle requests from DenoActor sent through the channel by calling the
/// appropriate function pointer passed to execute_function_with_shims and responding with the result.
///
/// The hierarchy of modules:
///
/// DenoExecutor -> DenoActor -> DenoModule
///              -> DenoActor -> DenoModule
///              -> DenoActor -> DenoModule
///               ...
pub struct DenoExecutor<C, M> {
    pub(crate) actor: DenoActor<C, M>,
}

#[async_trait]
pub trait CallbackProcessor<M> {
    async fn process_callback(&self, req: M);
}

#[async_trait]
impl CallbackProcessor<()> for () {
    async fn process_callback(&self, _req: ()) {}
}

impl<'a, C: Sync + Send + std::fmt::Debug + 'static, M: Sync + Send + 'static> DenoExecutor<C, M> {
    pub async fn execute(
        &self,
        method_name: &str,
        arguments: Vec<Arg>,
        call_context: C,
        callback_processor: impl CallbackProcessor<M>,
    ) -> Result<Value> {
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
                    callback_processor. process_callback(msg.expect("Channel was dropped before operation completion")).await;
                }

                final_result = &mut on_function_result => {
                    // function has resolved with the return value
                    break final_result;
                },
            }
        }
    }
}
