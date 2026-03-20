// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::fmt;
use thiserror::Error;

/// Wraps a database-driver-specific error (e.g., tokio_postgres::Error, mysql_async::Error).
/// Defined in core so it's database-agnostic; each backend provides `From<DriverError>`.
#[derive(Debug)]
pub struct DatabaseDriverError(pub Box<dyn std::error::Error + Send + Sync>);

impl fmt::Display for DatabaseDriverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for DatabaseDriverError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Failed to execute transaction {0}")]
    Transaction(String),

    #[error("Validation: {0}")]
    Validation(String),

    #[error("Driver: {0}")]
    Driver(#[from] DatabaseDriverError),

    #[error("Unable to load native certificates: {0}")]
    NativeCerts(#[from] std::io::Error),

    #[error("{0} {1}")]
    WithContext(String, #[source] Box<DatabaseError>),

    #[error("{0}")]
    BoxedError(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("Precheck: {0}")]
    Precheck(String),

    #[error("{0}")]
    Generic(String),
}

impl DatabaseError {
    /// Create a Driver error from any error type (e.g., tokio_postgres::Error).
    pub fn driver(e: impl std::error::Error + Send + Sync + 'static) -> DatabaseError {
        DatabaseError::Driver(DatabaseDriverError(Box::new(e)))
    }

    pub fn with_context(self, context: String) -> DatabaseError {
        DatabaseError::WithContext(context, Box::new(self))
    }
}

pub trait WithContext {
    fn with_context(self, context: String) -> Self;
}

impl<T> WithContext for Result<T, DatabaseError> {
    fn with_context(self, context: String) -> Result<T, DatabaseError> {
        self.map_err(|e| e.with_context(context))
    }
}
