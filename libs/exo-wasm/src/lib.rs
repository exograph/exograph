// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

/// This code has no concept of Exograph.
///
/// Module to encapsulate the logic creating a WASM module that supports
/// embedding.
mod wasm_error;
mod wasm_executor;
mod wasm_executor_pool;

pub use wasm_error::WasmError;
pub use wasm_executor_pool::WasmExecutorPool;
