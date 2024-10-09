// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use codemap_diagnostic::Diagnostic;
use std::path::Path;
use thiserror::Error;
use url::Url;

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

impl ModelBuildingError {
    pub fn tsjs_error(
        error: String,
        relative_source_path: &Path,
        cananical_source_path: &Url,
    ) -> Self {
        let relative_source_path_str = relative_source_path.to_str().unwrap();
        let canonical_source_path_str = cananical_source_path.to_string();

        // Replace cananical path ("file:///<user-directory>/project/src/foo.ts") with relative path ("src/foo.ts")
        let error = error
            .lines()
            .map(|line| line.replace(&canonical_source_path_str, relative_source_path_str))
            .collect::<Vec<String>>()
            .join("\n");

        ModelBuildingError::TSJSParsingError(error)
    }
}
