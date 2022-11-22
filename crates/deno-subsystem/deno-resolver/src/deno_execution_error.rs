use core_plugin_interface::core_resolver::{
    plugin::SubsystemResolutionError, system_resolver::SystemResolutionError,
};
use thiserror::Error;

use payas_deno::deno_error::DenoError;

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
}

impl DenoExecutionError {
    pub fn user_error_message(&self) -> Option<String> {
        match self {
            DenoExecutionError::Authorization => Some("Not authorized".to_string()),
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
