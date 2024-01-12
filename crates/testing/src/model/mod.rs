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

pub use operations_metadata::{
    build_operations_metadata, resolve_testvariable, OperationsMetadata,
};

/// Tests for a particular model
pub struct TestSuite {
    /// The directory containing src/ and tests/
    pub project_dir: PathBuf,
    pub tests: Vec<IntegrationTest>,
}

#[derive(Debug, Clone)]
pub struct IntegrationTest {
    pub testfile_path: PathBuf,
    pub retries: usize,
    pub init_operations: Vec<IntegrationTestOperation>,
    pub test_operations: Vec<IntegrationTestOperation>,
    pub extra_envs: HashMap<String, String>, // extra envvars ti be set when starting the exo server
}

#[derive(Debug, Clone)]
pub struct IntegrationTestOperation {
    pub document: String,
    pub operations_metadata: OperationsMetadata,
    pub variables: Option<String>,        // stringified
    pub expected_payload: Option<String>, // stringified
    pub deno_prelude: Option<String>,
    pub auth: Option<String>,    // stringified
    pub headers: Option<String>, // stringified
}
