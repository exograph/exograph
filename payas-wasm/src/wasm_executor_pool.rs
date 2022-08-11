use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use wasmtime::Val;

use crate::{wasm_error::WasmError, wasm_executor::WasmExecutor};

#[derive(Default)]
pub struct WasmExecutorPool {
    pub(crate) pool: Arc<Mutex<HashMap<String, WasmExecutor>>>,
}

impl WasmExecutorPool {
    pub async fn execute(
        &self,
        script_path: &str,
        script: &[u8],
        method_name: &str,
        arguments: Vec<Val>,
    ) -> Result<Value, WasmError> {
        let executor = self.get_executor(script_path, script)?;

        executor.execute(method_name, arguments)
    }

    fn get_executor(
        &self,
        module_name: &str,
        module_source: &[u8],
    ) -> Result<WasmExecutor, WasmError> {
        let mut pool = self.pool.lock().unwrap();
        let executor = pool
            .entry(module_name.to_string())
            .or_insert_with(|| WasmExecutor::new(module_source).unwrap());

        Ok(executor.clone())
    }
}
