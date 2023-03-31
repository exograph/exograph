/// This code has no concept of Exograph.
///
/// Module to encapsulate the logic creating a WASM module that supports
/// embedding.
mod wasm_error;
mod wasm_executor;
mod wasm_executor_pool;

pub use wasm_error::WasmError;
pub use wasm_executor_pool::WasmExecutorPool;
