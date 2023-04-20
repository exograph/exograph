// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Failed to execute transaction {0}")]
    Transaction(String),

    #[error("Validation: {0}")]
    Validation(String),

    #[error("Delegate: {0}")]
    Delegate(#[from] tokio_postgres::Error),

    #[error("SSL: {0}")]
    Ssl(#[from] openssl::error::ErrorStack),

    #[error("Pool: {0}")]
    Pool(#[from] deadpool_postgres::PoolError),

    #[error("{0} {1}")]
    WithContext(String, #[source] Box<DatabaseError>),

    #[error("{0}")]
    BoxedError(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl DatabaseError {
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
