use crate::wasm_error::WasmError;

use wasmtime::{Engine, Linker, Module, Store, Val};
use wasmtime_wasi::WasiCtxBuilder;

#[derive(Clone)]
pub struct WasmExecutor {
    module: Module,
}

impl WasmExecutor {
    pub fn new(module_source: &[u8]) -> Result<WasmExecutor, WasmError> {
        let engine = Engine::default();
        let module = Module::from_binary(&engine, module_source)?;

        Ok(WasmExecutor { module })
    }

    pub fn execute(
        &self,
        method_name: &str,
        arguments: Vec<Val>,
    ) -> Result<serde_json::Value, WasmError> {
        let mut linker = Linker::new(self.module.engine());
        wasmtime_wasi::add_to_linker(&mut linker, |s| s)?;

        let wasi = WasiCtxBuilder::new()
            .inherit_stdio()
            .inherit_args()?
            .build();

        let mut store = Store::new(self.module.engine(), wasi);

        linker.module(&mut store, "", &self.module)?;

        let func = linker
            .get(&mut store, "", method_name)
            .ok_or_else(|| WasmError::MethodNotFound(method_name.to_string()))?
            .into_func()
            .ok_or_else(|| WasmError::InvalidMethod(method_name.to_string()))?;

        let mut results = [0i32.into()];
        func.call(store, &arguments, &mut results)?;
        let result = &results[0];

        match result {
            wasmtime::Val::I32(n) => Ok((*n).into()),
            wasmtime::Val::I64(n) => Ok((*n).into()),
            wasmtime::Val::F32(n) => Ok((*n).into()),
            wasmtime::Val::F64(n) => Ok((*n).into()),
            wasmtime::Val::V128(_) => Err(WasmError::UnsupportedType("V128".to_string())),
            wasmtime::Val::FuncRef(_) => Err(WasmError::UnsupportedType("FuncRef".to_string())),
            wasmtime::Val::ExternRef(_) => Err(WasmError::UnsupportedType("ExternRef".to_string())),
        }
    }
}
