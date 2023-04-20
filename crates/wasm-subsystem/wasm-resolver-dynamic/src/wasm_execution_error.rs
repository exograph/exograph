// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use thiserror::Error;

use exo_wasm::WasmError;

#[derive(Error, Debug)]
pub enum WasmExecutionError {
    #[error("{0}")]
    Wasm(#[source] WasmError),

    #[error("Invalid argument {0}")]
    InvalidArgument(String),

    #[error("Not authorized")]
    Authorization,

    #[error("{0}")]
    Generic(String),

    #[error("{0}")]
    Delegate(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl WasmExecutionError {
    pub fn user_error_message(&self) -> String {
        match self {
            WasmExecutionError::Authorization => "Not authorized".to_string(),
            WasmExecutionError::Wasm(WasmError::Explicit(error)) => error.to_string(),
            WasmExecutionError::Delegate(error) => {
                match error.downcast_ref::<WasmExecutionError>() {
                    Some(error) => error.user_error_message(),
                    None => "Internal server error".to_string(),
                }
            }
            _ => "Internal server error".to_string(), // Do not reveal too much information about the error
        }
    }

    pub fn explicit_message(&self) -> Option<String> {
        fn root_error<'a>(
            error: &'a (dyn std::error::Error + 'static),
        ) -> &'a (dyn std::error::Error + 'static) {
            match error.source() {
                Some(source) => root_error(source),
                None => error,
            }
        }

        let root_error = root_error(self);
        match root_error.downcast_ref::<WasmError>() {
            Some(WasmError::Explicit(error)) => Some(error.to_string()),
            _ => None,
        }
    }
}
