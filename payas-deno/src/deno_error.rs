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
    #[error("Missing shim {0}")]
    MissingShim(String),
    #[error("No function named {0} exported from {1}")]
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
    Serde(#[from] serde_v8::Error),
    #[error("{0}")]
    DataError(#[from] DataError),
}
