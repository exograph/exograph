// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum WasmError {
    // Explicit error thrown by a script (this should be propagated to the user)
    #[error("{0}")]
    Explicit(String),

    #[error("{0}")]
    AnyError(#[from] anyhow::Error),

    #[error("{0}")]
    WasmtimeError(#[from] wasmtime::Error),

    #[error("Unsupported WASM type '{0}'")]
    UnsupportedType(String),

    #[error("{0}")]
    StringArrayError(#[from] wasi_common::StringArrayError),

    #[error("Failed to locate method '{0}'")]
    MethodNotFound(String),

    #[error("Failed to convert '{0}' to a WASM function")]
    InvalidMethod(String),
}
