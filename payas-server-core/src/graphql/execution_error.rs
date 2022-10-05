use payas_core_resolver::validation::validation_error::ValidationError;
use payas_database_resolver::DatabaseExecutionError;
use payas_wasm_resolver::WasmExecutionError;
use thiserror::Error;

use payas_deno_resolver::DenoExecutionError;

#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("{0}")]
    Generic(String),

    #[error("{0}")]
    Database(#[from] DatabaseExecutionError),

    #[error("{0}")]
    Deno(#[from] DenoExecutionError),

    #[error("{0}")]
    Wasm(#[from] WasmExecutionError),

    #[error("{0}")]
    Serde(#[from] serde_json::Error),

    #[error("{0}")]
    Validation(#[from] ValidationError),

    #[error("Invalid field {0} for {1}")]
    InvalidField(String, &'static str), // (field name, container type)
}
