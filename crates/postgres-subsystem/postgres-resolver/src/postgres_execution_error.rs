// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use thiserror::Error;

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
}

impl PostgresExecutionError {
    pub fn with_context(self, context: String) -> PostgresExecutionError {
        PostgresExecutionError::WithContext(context, Box::new(self))
    }

    pub fn user_error_message(&self) -> String {
        match self {
            PostgresExecutionError::Authorization => "Not authorized".to_string(),
            PostgresExecutionError::Validation(_, _) => self.to_string(),
            // Do not reveal the underlying database error as it may expose sensitive details (such as column names or data involved in constraint violation).
            _ => "Postgres operation failed".to_string(),
        }
    }
}
pub(crate) trait WithContext {
    fn with_context(self, context: String) -> Self;
}

impl<T> WithContext for Result<T, PostgresExecutionError> {
    fn with_context(self, context: String) -> Result<T, PostgresExecutionError> {
        self.map_err(|e| e.with_context(context))
    }
}
