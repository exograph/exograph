// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use codemap_diagnostic::Diagnostic;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParserError {
    // Don't include the source, because we emit is as a diagnostic
    #[error("Could not process input exo files")]
    Diagnosis(Vec<Diagnostic>),

    #[error("File '{0}' not found")]
    FileNotFound(String),

    #[error("{0}")]
    IO(#[from] std::io::Error),

    #[error("{0}")]
    ModelBuildingError(#[from] core_model_builder::error::ModelBuildingError),

    #[error("{0}")]
    ModelSerializationError(#[from] core_plugin_shared::error::ModelSerializationError),

    #[error("{0}")]
    Generic(String),

    #[error("Invalid trusted document format in file: '{0}'")]
    InvalidTrustedDocumentFormat(String),

    #[error("No trusted documents found in directory: '{0}'. No queries or mutation will be allowed in production mode.")]
    NoTrustedDocuments(String),
}
