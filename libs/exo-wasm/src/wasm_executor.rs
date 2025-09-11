// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::wasm_error::WasmError;

use wasi_common::sync::WasiCtxBuilder;
use wasmtime::{Engine, Linker, Module, Store, Val};

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
        wasi_common::sync::add_to_linker(&mut linker, |s| s)?;

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
            wasmtime::Val::AnyRef(_) => Err(WasmError::UnsupportedType("AnyRef".to_string())),
            wasmtime::Val::ExnRef(_) => Err(WasmError::UnsupportedType("ExnRef".to_string())),
        }
    }
}
