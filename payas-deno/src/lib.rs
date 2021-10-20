mod deno_module;
mod embedded_module_loader;

use anyhow::Result;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::{
    collections::HashMap,
    ops::Deref,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub use deno_module::{Arg, DenoModule};

type DenoModuleRpc = (
    Sender<(String, Vec<Arg>)>,
    Receiver<Result<serde_json::Value>>,
);

#[derive(Default)]
pub struct DenoModulesMap {
    module_map: HashMap<PathBuf, Arc<Mutex<DenoModuleRpc>>>,
}
type RpcChannel = (Sender<(String, Vec<Arg>)>, Receiver<(String, Vec<Arg>)>);

impl DenoModulesMap {
    pub fn new() -> DenoModulesMap {
        DenoModulesMap::default()
    }

    pub fn load_module(&mut self, module_path: &Path) -> Result<()> {
        if !self.module_map.contains_key(module_path) {
            let (rpc_sender, rpc_receiver): RpcChannel = channel();
            let (value_sender, value_receiver) = channel();
            let path = module_path.to_path_buf();

            std::thread::spawn(move || {
                let shims = vec![];

                let mut module =
                    futures::executor::block_on(DenoModule::new(&path, "Claytip", &shims, |_| {}))
                        .unwrap();

                loop {
                    let (method_name, args) = rpc_receiver.recv().unwrap();

                    module.preload_function(vec![&method_name]);
                    let val =
                        futures::executor::block_on(module.execute_function(&method_name, args));

                    value_sender.send(val).unwrap()
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
    ) -> Result<serde_json::Value> {
        let mutex = &self.module_map[module_path];
        let ptr = mutex.lock().unwrap();
        let (rpc_sender, value_receiver) = ptr.deref();
        rpc_sender.send((method_name.to_owned(), args)).unwrap();
        value_receiver.recv().unwrap()
    }
}
