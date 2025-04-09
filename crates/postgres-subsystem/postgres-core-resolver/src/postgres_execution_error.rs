// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use common::context::ContextExtractionError;
use core_resolver::{access_solver::AccessSolverError, plugin::SubsystemResolutionError};

use thiserror::Error;
use tracing::error;

use super::cast::CastError;

#[derive(Error, Debug)]
pub enum PostgresExecutionError {
    #[error("{0}")]
    Generic(String),

    #[error("Invalid field '{0}': {1}")]
    Validation(String, String),

    #[error("{0}")]
    Postgres(#[from] exo_sql::database_error::DatabaseError),

    #[error("{0}")]
    EmptyRow(#[from] tokio_postgres::Error),

    #[error("Result has {0} entries; expected only zero or one")]
    NonUniqueResult(usize),

    #[error("{0}")]
    CastError(#[from] CastError),

    #[error("Not authorized")]
    Authorization,

    #[error("{0} {1}")]
    WithContext(String, #[source] Box<PostgresExecutionError>),

    #[error("Missing argument '{0}'")]
    MissingArgument(String),

    #[error("{0}")]
    ContextExtraction(#[from] ContextExtractionError),
}

impl PostgresExecutionError {
    pub fn with_context(self, context: String) -> PostgresExecutionError {
        PostgresExecutionError::WithContext(context, Box::new(self))
    }

    pub fn user_error_message(&self) -> String {
        match self {
            PostgresExecutionError::Authorization => "Not authorized".to_string(),
            PostgresExecutionError::Validation(_, _) => self.to_string(),
            PostgresExecutionError::CastError(e) => {
                error!("Cast error: {}", e);
                "Unable to convert input to the expected type".to_string()
            }
            PostgresExecutionError::WithContext(context, e) => {
                format!("{}: {}", e.user_error_message(), context)
            }
            // Do not reveal the underlying database error as it may expose sensitive details (such as column names or data involved in constraint violation).
            _ => {
                error!("Postgres operation failed: {:?}", self);
                "Operation failed".to_string()
            }
        }
    }
}

impl From<AccessSolverError> for PostgresExecutionError {
    fn from(error: AccessSolverError) -> Self {
        match error {
            AccessSolverError::ContextExtraction(ce) => {
                PostgresExecutionError::ContextExtraction(ce)
            }
            _ => PostgresExecutionError::Generic(error.to_string()),
        }
    }
}

pub trait WithContext {
    fn with_context(self, context: String) -> Self;
}

impl<T> WithContext for Result<T, PostgresExecutionError> {
    fn with_context(self, context: String) -> Result<T, PostgresExecutionError> {
        self.map_err(|e| e.with_context(context))
    }
}

impl From<PostgresExecutionError> for SubsystemResolutionError {
    fn from(e: PostgresExecutionError) -> Self {
        match e {
            PostgresExecutionError::Authorization => SubsystemResolutionError::Authorization,
            PostgresExecutionError::ContextExtraction(ce) => {
                SubsystemResolutionError::ContextExtraction(ce)
            }
            _ => SubsystemResolutionError::UserDisplayError(e.user_error_message()),
        }
    }
}
