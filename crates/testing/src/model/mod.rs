// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{collections::HashMap, path::PathBuf};

mod operations_metadata;

pub use operations_metadata::{OperationMetadata, build_operations_metadata, resolve_testvariable};

/// Tests for a particular model
pub struct TestSuite {
    /// The directory containing src/ and tests/
    pub project_dir: PathBuf,
    pub tests: Vec<IntegrationTest>,
}

#[derive(Debug, Clone)]
pub struct IntegrationTest {
    /// The root directory from which the test command was run (used to compute the test name to be relative to the root directory)
    pub root_dir: PathBuf,
    /// The exo project directory containing src/ and tests/ (used to compute the test name to drop the "tests/" prefix)
    pub project_dir: PathBuf,
    pub testfile_path: PathBuf,
    pub retries: usize,
    pub init_operations: Vec<InitOperation>,
    pub test_operations: Vec<ApiOperation>,
    /// Extra envvars to be set when starting the exo server
    pub extra_envs: HashMap<String, String>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum InitOperation {
    Database(DatabaseOperation),
    Api(ApiOperation),
}

#[derive(Debug, Clone)]
pub struct DatabaseOperation {
    pub sql: String, // SQL statements separated by semicolons
}

#[derive(Debug, Clone)]
pub struct ApiOperation {
    pub document: String,
    pub metadata: OperationMetadata,
    pub variables: Option<String>, // stringified
    pub deno_prelude: Option<String>,
    pub auth: Option<String>,    // stringified
    pub headers: Option<String>, // stringified

    pub expected_response: Option<String>, // stringified
    pub invariants: Vec<ApiOperationInvariant>,
}

#[derive(Debug, Clone)]
pub struct ApiOperationInvariant {
    pub operation: ApiOperation,
}
