/// This code has no concept of Claytip.
///
/// Module to encapsulate the logic creating a WASM module that supports
/// embedding.
mod wasm_error;
mod wasm_executor;
mod wasm_executor_pool;

pub use wasm_executor_pool::WasmExecutorPool;
