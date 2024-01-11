// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use async_graphql_parser::parse_query;
use serde::Deserialize;

use crate::model::{
    build_operations_metadata, IntegrationTest, IntegrationTestOperation, OperationsMetadata,
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
        let relative_testfile_path = {
            let base_path = self.testfile_path.components().skip(1).collect::<PathBuf>();

            if base_path.starts_with("tests") {
                base_path.components().skip(1).collect::<PathBuf>()
            } else {
                base_path
            }
        };

        // Drop to extension (".exotest") to obtain the name
        relative_testfile_path
            .with_extension("")
            .to_str()
            .expect("Failed to convert file name into Unicode")
            .to_string()
    }

    pub fn exo_ir_file_path(&self, project_dir: &Path) -> PathBuf {
        project_dir.join("target").join("index.exo_ir")
    }

    pub fn load(
        testfile_path: &PathBuf,
        init_ops: Vec<IntegrationTestOperation>,
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
                        OperationsMetadata::default()
                    });

                Ok(IntegrationTestOperation {
                    document: stage.operation,
                    operations_metadata,
                    auth: stage.auth,
                    variables: stage.variable,
                    expected_payload: stage.response,
                    headers: stage.headers,
                    deno_prelude: stage.deno,
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
        })
    }
}
