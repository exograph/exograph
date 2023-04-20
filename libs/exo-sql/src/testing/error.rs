// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum EphemeralDatabaseSetupError {
    #[error("Failed to start postgres")]
    PostgresFailedToStart(#[from] io::Error),
    #[error("Failed to find executable")]
    ExecutableNotFound(#[from] which::Error),

    #[error("Failed to start Docker (it may not be installed) {0}")]
    Docker(#[source] io::Error),

    #[error("Failed to start postgres {0}")]
    Generic(String),
}
