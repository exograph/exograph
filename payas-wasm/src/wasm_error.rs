use thiserror::Error;

#[derive(Error, Debug)]
pub enum WasmError {
    // Explicit error thrown by a script (this should be propagated to the user)
    #[error("{0}")]
    Explicit(String),

    #[error("{0}")]
    AnyError(#[from] anyhow::Error),

    #[error("Unsupported WASM type '{0}'")]
    UnsupportedType(String),

    #[error("{0}")]
    StringArrayError(#[from] wasi_common::StringArrayError),

    #[error("Failed to locate method '{0}'")]
    MethodNotFound(String),

    #[error("Failed to convert '{0}' to a WASM function")]
    InvalidMethod(String),
}
