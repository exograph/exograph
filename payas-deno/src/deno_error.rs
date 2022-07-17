use deno_core::{error::AnyError, v8::DataError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DenoError {
    #[error("Channel error")]
    Channel,
    #[error("{0}")]
    Generic(String),
    #[error(transparent)]
    Delegate(#[from] AnyError),
    #[error(transparent)]
    BorrowMutError(#[from] core::cell::BorrowMutError),
    #[error(transparent)]
    SerdeError(#[from] serde_v8::Error),
    #[error(transparent)]
    DataError(#[from] DataError),
}
