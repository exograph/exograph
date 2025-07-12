// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use common::context::ContextExtractionError;
use core_resolver::{
    access_solver::AccessSolverError, plugin::SubsystemResolutionError,
    system_resolver::SystemResolutionError,
};
use thiserror::Error;

use exo_deno::error::DenoError;

#[derive(Error, Debug)]
pub enum DenoExecutionError {
    #[error("{0}")]
    Deno(#[source] DenoError),

    #[error("Invalid argument {0}")]
    InvalidArgument(String),

    #[error("Not authorized")]
    Authorization,

    #[error("{0}")]
    Generic(String),

    #[error("{0}")]
    ContextExtraction(#[from] ContextExtractionError),
}

impl DenoExecutionError {
    pub fn user_error_message(&self) -> Option<String> {
        match self {
            DenoExecutionError::Authorization => Some("Not authorized".to_string()),
            DenoExecutionError::ContextExtraction(ce) => Some(ce.user_error_message()),
            DenoExecutionError::Deno(DenoError::Explicit(error)) => Some(error.to_string()),
            _ => self.explicit_message(),
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

        // To deal with nested exceptions, we need to check if it is an explicit error (direct
        // invocation, so DenoError::Explicit suffices), or did it invoke it in nested fashion
        // (indirect invocation, SubsystemResolutionError), or it invoked another subsystem (cross
        // invocation, SystemResolutionError).
        match root_error.downcast_ref::<DenoError>() {
            Some(DenoError::Explicit(error)) => Some(error.to_string()),
            _ => match root_error.downcast_ref::<SubsystemResolutionError>() {
                Some(error) => error.user_error_message(),
                _ => root_error
                    .downcast_ref::<SystemResolutionError>()
                    .map(|error| error.user_error_message()),
            },
        }
    }
}

impl From<AccessSolverError> for DenoExecutionError {
    fn from(error: AccessSolverError) -> Self {
        match error {
            AccessSolverError::ContextExtraction(e) => DenoExecutionError::ContextExtraction(e),
            _ => DenoExecutionError::Generic(error.to_string()),
        }
    }
}
