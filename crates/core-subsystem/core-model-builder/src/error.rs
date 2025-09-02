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

use core_plugin_shared::error::ModelSerializationError;

#[derive(Error, Debug)]
pub enum ModelBuildingError {
    #[error("Could not process input exo files")]
    Diagnosis(Vec<Diagnostic>),

    #[error("{0}")]
    IO(#[from] std::io::Error),

    #[error("{0}")]
    Generic(String),

    #[error("Could not parse TypeScript/JavaScript files")]
    TSJSParsingError(String),

    #[error("Unable to serialize model {0}")]
    Serialize(#[source] ModelSerializationError),

    #[error("Unable to deserialize model {0}")]
    Deserialize(#[source] ModelSerializationError),
}
