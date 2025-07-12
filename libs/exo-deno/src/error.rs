// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use deno_core::{
    error::{AnyError, JsError},
    v8::DataError,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DenoError {
    // Explicit error thrown by a script (this should be propagated to the user)
    #[error("{0}")]
    Explicit(String),

    // Non-explicit error thrown by the runtime (such as undefined variable)
    #[error("{0}")]
    JsError(#[from] JsError),

    #[error("{0}")]
    AnyError(#[from] AnyError),

    // Show it to developers (such as missing "await") so they may possibly fix it.
    #[error("{0}")]
    Diagnostic(#[from] DenoDiagnosticError),

    // An issue with this crate or a dependency
    #[error("{0}")]
    Internal(#[from] DenoInternalError),
}

#[derive(Error, Debug)]
pub enum DenoDiagnosticError {
    #[error("Missing shim `{0}`")]
    MissingShim(String),
    #[error("No function named `{0}` exported from {1}")]
    MissingFunction(String, String), // (function name, module name)

    #[error("{0}")]
    BorrowMutError(#[from] core::cell::BorrowMutError), // Diagnostic for now: possibly missing "await"
}

#[derive(Error, Debug)]
pub enum DenoInternalError {
    #[error("{0}")]
    Channel(String),

    #[error("{0}")]
    Any(#[from] AnyError),
    #[error("{0}")]
    Serde(#[from] deno_core::serde_v8::Error),
    #[error("{0}")]
    DataError(#[from] DataError),

    #[error("{0}")]
    CoreError(#[from] deno_core::error::CoreError),
}
