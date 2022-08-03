use thiserror::Error;

use payas_deno::deno_error::DenoError;

#[derive(Error, Debug)]
pub enum DenoExecutionError {
    #[error(transparent)]
    Deno(#[from] DenoError),

    #[error("Invalid argument {0}")]
    InvalidArgument(String),

    #[error("Not authorized")]
    Authorization,
}
