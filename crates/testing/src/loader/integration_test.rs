// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use async_graphql_parser::parse_query;
use serde::Deserialize;

use crate::model::{
    ApiOperation, ApiOperationInvariant, DatabaseOperation, InitOperation, IntegrationTest,
    OperationMetadata, build_operations_metadata,
};

// serde file formats
#[derive(Deserialize, Debug, Clone)]
struct TestfileStage {
    pub headers: Option<String>,
    pub deno: Option<String>,
    pub operation: String,
    pub variable: Option<String>,
    pub auth: Option<String>,
    pub response: Option<String>,
    pub invariants: Option<Vec<TestfileStageInvariant>>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

#[derive(Deserialize, Debug, Clone)]
struct TestfileStageInvariant {
    // Path the invariant file (relative to the testfile)
    pub path: String,
    // TODO: Allow overriding headers, auth, variables, etc.
}

#[derive(Deserialize, Debug)]
struct TestfileCommon {
    #[serde(default)]
    pub retries: usize,
    pub envs: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Debug)]
struct TestfileSingleStage {
    #[serde(flatten)]
    pub common: TestfileCommon,
    #[serde(flatten)]
    pub stage: TestfileStage,
}

#[derive(Deserialize, Debug)]
struct TestfileMultipleStages {
    #[serde(flatten)]
    pub common: TestfileCommon,
    pub stages: Vec<TestfileStage>,
}

impl IntegrationTest {
    pub fn name(&self) -> String {
        // Make the test path relative to the root directory where the test command was run
        let relative_testfile_path = self
            .testfile_path
            .strip_prefix(&self.root_dir)
            .unwrap_or(&self.testfile_path);

        // Only remove "tests" directory if it's a direct child of the project directory
        let project_tests_dir = self.project_dir.join("tests");
        let path_without_project_tests = if self.testfile_path.starts_with(&project_tests_dir) {
            // If the test file is under project_dir/tests/, remove that "tests" part
            self.testfile_path
                .strip_prefix(&project_tests_dir)
                .map(|p| {
                    // Make it relative to root_dir
                    let project_relative = self
                        .project_dir
                        .strip_prefix(&self.root_dir)
                        .unwrap_or(&self.project_dir);
                    project_relative.join(p)
                })
                .unwrap_or(relative_testfile_path.to_path_buf())
        } else {
            relative_testfile_path.to_path_buf()
        };

        // Drop the extension (".exotest") to obtain the name
        path_without_project_tests
            .with_extension("")
            .to_str()
            .expect("Failed to convert file name into Unicode")
            .to_string()
    }

    pub fn exo_ir_file_path(&self, project_dir: &Path) -> PathBuf {
        project_dir.join("target").join("index.exo_ir")
    }

    pub fn load_init_operations(init_file_path: &PathBuf) -> Result<Vec<InitOperation>> {
        let extension = init_file_path
            .extension()
            .ok_or(anyhow::anyhow!("Init file has no extension"))?
            .to_str()
            .ok_or(anyhow::anyhow!("Init file extension is not valid UTF-8"))?;

        if extension == "gql" {
            // For init files, we don't care about the name, so we use the parent directory as root and project
            let parent = init_file_path.parent().unwrap();
            Self::load(init_file_path, vec![], parent, parent).map(|test| {
                let IntegrationTest {
                    test_operations, ..
                } = test;
                test_operations
                    .into_iter()
                    .map(InitOperation::Api)
                    .collect()
            })
        } else if extension == "sql" {
            let mut file = File::open(init_file_path).context("Could not open init file")?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)
                .context("Could not read init file to string")?;

            Ok(vec![InitOperation::Database(DatabaseOperation {
                sql: contents,
            })])
        } else {
            bail!("Unsupported init file extension: {}", extension);
        }
    }

    pub fn load(
        testfile_path: &PathBuf,
        init_ops: Vec<InitOperation>,
        root_dir: &Path,
        project_dir: &Path,
    ) -> Result<IntegrationTest> {
        let mut file = File::open(testfile_path).context("Could not open test file")?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .context("Could not read test file to string")?;

        let deserialized_testfile_multiple_stages: Result<TestfileMultipleStages, _> =
            serde_yaml::from_str(&contents);
        let deserialized_testfile_single_stage: Result<TestfileSingleStage, _> =
            serde_yaml::from_str(&contents);

        let (stages, common) = if let Ok(testfile) = deserialized_testfile_multiple_stages {
            (testfile.stages, testfile.common)
        } else if let Ok(testfile) = deserialized_testfile_single_stage {
            (vec![testfile.stage.clone()], testfile.common)
        } else {
            let multi_stage_error = deserialized_testfile_multiple_stages.unwrap_err();
            let single_stage_error = deserialized_testfile_single_stage.unwrap_err();

            bail!(
                r#"
    Could not deserialize testfile at {} as a single operation test nor as a multistage one.
    
    Error as a single stage test: {}
    Error as a multistage test: {}
    "#,
                testfile_path.to_str().unwrap(),
                single_stage_error,
                multi_stage_error
            );
        };

        let extra_keys = stages
            .iter()
            .flat_map(|stage| stage.extra.keys())
            .collect::<HashSet<_>>();

        if !extra_keys.is_empty() {
            bail!(
                "Unknown fields: {:?} defined in {}",
                extra_keys.iter().collect::<Vec<_>>(),
                testfile_path.to_str().unwrap()
            );
        }

        // validate GraphQL
        let test_operation_sequence = stages
            .into_iter()
            .map(|stage| {
                let operations_metadata = parse_query(&stage.operation)
                    .map(|gql_document| build_operations_metadata(&gql_document))
                    .unwrap_or_else(|_| {
                        eprintln!(
                            "Invalid GraphQL document; defaulting test variables binding to empty"
                        );
                        OperationMetadata::default()
                    });

                let invariants =
                    Self::load_invariants(testfile_path, stage.invariants.unwrap_or_default())?;

                Ok(ApiOperation {
                    document: stage.operation,
                    metadata: operations_metadata,
                    auth: stage.auth,
                    variables: stage.variable,
                    expected_response: stage.response,
                    headers: stage.headers,
                    deno_prelude: stage.deno,
                    invariants,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        assert!(common.retries <= 5, "The maximum number of retries is 5");

        Ok(IntegrationTest {
            testfile_path: testfile_path.to_path_buf(),
            retries: common.retries,
            extra_envs: common.envs.unwrap_or_default(),
            init_operations: init_ops,
            test_operations: test_operation_sequence,
            root_dir: root_dir.to_path_buf(),
            project_dir: project_dir.to_path_buf(),
        })
    }

    fn load_invariants(
        testfile_path: &Path,
        invariants: Vec<TestfileStageInvariant>,
    ) -> Result<Vec<ApiOperationInvariant>> {
        let testfile_dir = testfile_path.parent().unwrap();

        let mut invariant_ops: Vec<ApiOperationInvariant> = vec![];

        for invariant in invariants {
            let invariant_path = testfile_dir.join(invariant.path.clone());
            // For invariants, we don't care about the name, so we use the parent directory as root and project
            let parent = invariant_path.parent().unwrap();
            let integration_test = Self::load(&invariant_path, vec![], parent, parent)?;

            for op in integration_test.test_operations {
                invariant_ops.push(ApiOperationInvariant { operation: op });
            }
        }

        Ok(invariant_ops)
    }
}
