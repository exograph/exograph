// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContextExtractionError {
    #[error("Could not find source `{0}`")]
    SourceNotFound(String),

    #[error("Unauthorized request")]
    Unauthorized,

    #[error("Expired authentication")]
    ExpiredAuthentication,

    #[error("Malformed request")]
    Malformed,

    #[error("{0}")]
    Delegate(#[from] Box<dyn std::error::Error + Send + Sync>),

    #[error("Type mismatch: Expected `{expected}`, found `{actual}`")]
    TypeMismatch { expected: String, actual: String },

    #[error("Field not found: `{0}`")]
    FieldNotFound(String),

    #[error("Unexpected function call in context selection: `{0}`")]
    UnexpectedFunctionCallInContextSelection(String),

    #[error("{0}")]
    Generic(String),
}

impl ContextExtractionError {
    pub fn user_error_message(&self) -> String {
        match self {
            ContextExtractionError::ExpiredAuthentication => "Expired authentication".to_string(),
            _ => "Not authorized".to_string(),
        }
    }
}
