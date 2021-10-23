mod deno_module;
mod embedded_module_loader;

use anyhow::{anyhow, Result};
use crossbeam_channel::{unbounded, Receiver, Sender};
use serde_json::Value;
use std::{
    collections::HashMap,
    ops::Deref,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub use deno_module::{Arg, DenoModule};

type DenoModuleRpc = (Sender<ToDenoMessage>, Receiver<FromDenoMessage>);

#[derive(Default)]
pub struct DenoModulesMap {
    module_map: HashMap<PathBuf, Arc<Mutex<DenoModuleRpc>>>,
}

type RpcChannel = (Sender<ToDenoMessage>, Receiver<ToDenoMessage>);

pub enum ToDenoMessage {
    RequestMethodCall(String, Vec<Arg>),

    ResponseClaytipExecute(Result<Value>),
}

pub enum FromDenoMessage {
    ResponseMethodCall(Result<Value>),

    RequestClaytipExecute {
        query_string: String,
        variables: Option<serde_json::Map<String, Value>>,
    },
}

impl DenoModulesMap {
    pub fn new() -> DenoModulesMap {
        DenoModulesMap::default()
    }

    pub fn load_module(&mut self, module_path: &Path) -> Result<()> {
        if !self.module_map.contains_key(module_path) {
            let (rpc_sender, rpc_receiver): RpcChannel = unbounded();
            let (value_sender, value_receiver) = unbounded();
            let path = module_path.to_path_buf();

            std::thread::spawn(move || {
                let shims = vec![("ClaytipInjected", include_str!("claytip_shim.js"))];

                let to_claytip = value_sender.clone();
                let from_claytip = rpc_receiver.clone();

                let mut module = futures::executor::block_on(DenoModule::new(
                    &path,
                    "Claytip",
                    &shims,
                    &move |runtime| {
                        let claytip_sender = to_claytip.clone();
                        let claytip_receiver = from_claytip.clone();

                        let sync_ops = vec![(
                            "op_claytip_execute_query",
                            deno_core::op_sync(move |_state, args: Vec<String>, _: ()| {
                                let query_string = &args[0];
                                let variables: Option<serde_json::Map<String, Value>> =
                                    args.get(1).map(|vars| serde_json::from_str(vars).unwrap());

                                claytip_sender
                                    .send(FromDenoMessage::RequestClaytipExecute {
                                        query_string: query_string.to_owned(),
                                        variables,
                                    })
                                    .unwrap();

                                if let ToDenoMessage::ResponseClaytipExecute(result) =
                                    claytip_receiver.recv().unwrap()
                                {
                                    result
                                } else {
                                    panic!()
                                }
                            }),
                        )];
                        for (name, op) in sync_ops {
                            runtime.register_op(name, op);
                        }
                    },
                ))
                .unwrap();

                loop {
                    if let ToDenoMessage::RequestMethodCall(method_name, args) =
                        rpc_receiver.recv().unwrap()
                    {
                        module.preload_function(vec![&method_name]);
                        let val = futures::executor::block_on(
                            module.execute_function(&method_name, args),
                        );

                        value_sender
                            .send(FromDenoMessage::ResponseMethodCall(val))
                            .unwrap()
                    }
                }
            });

            self.module_map.insert(
                module_path.to_path_buf(),
                Arc::new(Mutex::new((rpc_sender, value_receiver))),
            );
        }

        Ok(())
    }

    pub fn execute_function(
        &mut self,
        module_path: &Path,
        method_name: &str,
        args: Vec<Arg>,
        // TODO: this should become a context struct?
        execute_query: &dyn Fn(String, Option<&serde_json::Map<String, Value>>) -> Result<Value>,
    ) -> Result<serde_json::Value> {
        let mutex = &self.module_map[module_path];
        let ptr = mutex
            .try_lock()
            .map_err(|_| anyhow!("Trying to executeQuery a method from the same module!"))?;
        let (rpc_sender, value_receiver) = ptr.deref();
        rpc_sender
            .send(ToDenoMessage::RequestMethodCall(
                method_name.to_owned(),
                args,
            ))
            .unwrap();

        // state machine
        loop {
            match value_receiver.recv().unwrap() {
                FromDenoMessage::ResponseMethodCall(val) => return val,
                FromDenoMessage::RequestClaytipExecute {
                    query_string,
                    variables,
                } => {
                    let result = execute_query(query_string, variables.as_ref());
                    rpc_sender
                        .send(ToDenoMessage::ResponseClaytipExecute(result))
                        .unwrap()
                }
            }
        }
    }
}
