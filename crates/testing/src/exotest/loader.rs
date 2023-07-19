// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use wildmatch::WildMatch;

use anyhow::{bail, Context, Result};
use async_graphql_parser::parse_query;

use crate::exotest::testvariable_bindings::OperationsMetadata;

use super::testvariable_bindings::build_operations_metadata;

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum TestfileOperation {
    Sql(String),
    GqlDocument {
        document: String,
        operations_metadata: OperationsMetadata,
        variables: Option<String>,        // stringified
        expected_payload: Option<String>, // stringified
        deno_prelude: Option<String>,
        auth: Option<serde_json::Value>,
        headers: Option<String>, // stringified
    },
}

pub struct ProjectTests {
    pub project_dir: PathBuf,
    pub tests: Vec<ParsedTestfile>,
}

#[derive(Debug, Clone)]
pub struct ParsedTestfile {
    testfile_path: PathBuf,
    pub retries: usize,
    pub init_operations: Vec<TestfileOperation>,
    pub extra_envs: HashMap<String, String>, // extra envvars to set for the entire testfile
    pub test_operation_stages: Vec<TestfileOperation>,
}

impl ParsedTestfile {
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
}

// serde file formats

#[derive(Deserialize, Debug, Clone)]
pub struct TestfileStage {
    pub exofile: Option<String>,
    pub headers: Option<String>,
    pub deno: Option<String>,
    pub operation: String,
    pub variable: Option<String>,
    pub auth: Option<String>,
    pub response: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct TestfileCommon {
    pub exofile: Option<String>,
    #[serde(default)]
    pub retries: usize,
    pub envs: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Debug)]
pub struct TestfileSingleStage {
    #[serde(flatten)]
    pub common: TestfileCommon,
    #[serde(flatten)]
    pub stage: TestfileStage,
}

#[derive(Deserialize, Debug)]
pub struct TestfileMultipleStages {
    #[serde(flatten)]
    pub common: TestfileCommon,
    pub stages: Vec<TestfileStage>,
}

#[derive(Deserialize, Debug)]
pub struct InitFile {
    pub operation: String,
    pub variable: Option<String>,
    pub auth: Option<String>,
    pub headers: Option<String>,
    pub deno: Option<String>,
}

fn collect_exo_projects(root_directory: &Path) -> Vec<PathBuf> {
    fn helper(dir: &Path, acc: &mut Vec<PathBuf>) {
        for subdir in dir.read_dir().unwrap().flatten() {
            if subdir.path().is_dir() {
                let subdir_path = subdir.path();
                if is_exoproject_with_tests(&subdir_path) {
                    acc.push(subdir_path);
                } else {
                    helper(&subdir_path, acc);
                }
            }
        }
    }

    let mut exo_projects = vec![];
    helper(root_directory, &mut exo_projects);
    exo_projects
}

/// Load and parse testfiles from a given directory.
pub fn load_project_dir(
    root_directory: &PathBuf,
    pattern: &Option<String>,
) -> Result<Vec<ProjectTests>> {
    let exo_project_dirs = if is_exoproject_with_tests(root_directory) {
        // If the root directory is an exo project, and it has tests, then we load the tests from it
        // This will be typical for user projects
        vec![root_directory.to_owned()]
    } else {
        // This is typical for the exo repo itself (and a multi-project repo)
        collect_exo_projects(root_directory)
    };

    exo_project_dirs
        .into_iter()
        .map(|exo_project_dir| {
            let tests = load_tests_dir(&exo_project_dir, &[], pattern)?;
            Ok(ProjectTests {
                project_dir: exo_project_dir,
                tests,
            })
        })
        .collect::<Result<Vec<_>>>()
}

fn load_tests_dir(
    test_directory: &Path, // directory that contains "src/index.exo"
    init_ops: &[TestfileOperation],
    pattern: &Option<String>,
) -> Result<Vec<ParsedTestfile>> {
    // Begin directory traversal
    let mut exotest_files: Vec<PathBuf> = vec![];
    let mut init_files: Vec<PathBuf> = vec![];
    let mut sub_directories: Vec<PathBuf> = vec![];

    for dir_entry in (test_directory.read_dir()?).flatten() {
        if dir_entry.path().is_file() {
            if let Some(extension) = dir_entry.path().extension() {
                // looking for .exotest files in our current directory
                if extension == "exotest" {
                    exotest_files.push(dir_entry.path());
                }

                // looking for init* files in our current directory
                if let Some(filename) = dir_entry.path().file_name() {
                    // TODO: https://github.com/rust-lang/rust/issues/49802
                    //if filename.starts_with("init") {
                    if filename.to_str().unwrap().starts_with("init")
                        && (extension == "sql" || extension == "gql")
                    {
                        init_files.push(dir_entry.path());
                    }
                }
            }
        } else if dir_entry.path().is_dir() {
            sub_directories.push(dir_entry.path())
        }
    }

    // sort init files lexicographically
    init_files.sort();

    // Parse init files and populate init_ops
    let mut init_ops = init_ops.to_owned();

    for initfile_path in init_files.iter() {
        let init_op = construct_operation_from_init_file(initfile_path)?;
        init_ops.push(init_op);
    }

    // Parse test files
    let mut testfiles = vec![];

    for testfile_path in exotest_files.iter() {
        let testfile = parse_testfile(testfile_path, init_ops.clone())?;
        testfiles.push(testfile);
    }

    // Recursively parse test files
    for sub_directory in sub_directories.iter() {
        let child_init_ops = init_ops.clone();
        let child_testfiles = load_tests_dir(sub_directory, &child_init_ops, pattern)?;
        testfiles.extend(child_testfiles)
    }

    let filtered_testfiles = match pattern {
        Some(pattern) => {
            let wildcard = WildMatch::new(pattern);
            testfiles
                .into_iter()
                .filter(|testfile| wildcard.matches(&testfile.name()))
                .collect()
        }
        None => testfiles,
    };

    Ok(filtered_testfiles)
}

fn parse_testfile(
    testfile_path: &PathBuf,
    init_ops: Vec<TestfileOperation>,
) -> Result<ParsedTestfile> {
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

            Ok(TestfileOperation::GqlDocument {
                document: stage.operation,
                operations_metadata,
                auth: stage.auth.map(from_json).transpose()?,
                variables: stage.variable,
                expected_payload: stage.response,
                headers: stage.headers,
                deno_prelude: stage.deno,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    assert!(common.retries <= 5, "The maximum number of retries is 5");

    Ok(ParsedTestfile {
        testfile_path: testfile_path.to_path_buf(),
        retries: common.retries,
        extra_envs: common.envs.unwrap_or_default(),
        init_operations: init_ops,
        test_operation_stages: test_operation_sequence,
    })
}

fn construct_operation_from_init_file(path: &Path) -> Result<TestfileOperation> {
    match path.extension().unwrap().to_str().unwrap() {
        "sql" => {
            let sql = std::fs::read_to_string(path).context("Failed to read SQL file")?;

            Ok(TestfileOperation::Sql(sql))
        }
        "gql" => {
            let file = File::open(path)?;
            let reader = BufReader::new(file);
            let deserialized_initfile: InitFile =
                serde_yaml::from_reader(reader).context(format!("Failed to parse {path:?}"))?;

            // validate GraphQL
            let gql_document =
                parse_query(&deserialized_initfile.operation).context("Invalid GraphQL")?;

            Ok(TestfileOperation::GqlDocument {
                document: deserialized_initfile.operation.clone(),
                operations_metadata: build_operations_metadata(&gql_document),
                auth: deserialized_initfile.auth.map(from_json).transpose()?,
                variables: deserialized_initfile.variable,
                headers: deserialized_initfile.headers,
                expected_payload: None,
                deno_prelude: deserialized_initfile.deno,
            })
        }
        _ => {
            bail!("Bad extension")
        }
    }
}

// Parse JSON from a string
fn from_json(json: String) -> Result<serde_json::Value> {
    serde_json::from_str(&json).context("Failed to parse JSON")
}

// Exograph projects have a src/index.exo file
fn is_exoproject_with_tests(dir: &Path) -> bool {
    directory_contains(dir, "src", true)
        && {
            let src_dir = dir.join("src");
            directory_contains(&src_dir, "index.exo", false)
        }
        && directory_contains(dir, "tests", true)
}

fn directory_contains(dir: &Path, name: &str, is_dir: bool) -> bool {
    if !dir.is_dir() {
        return false;
    }

    let dir_entry = dir.read_dir().unwrap().flatten().find(|dir_entry| {
        dir_entry.file_name() == name && dir_entry.file_type().unwrap().is_dir() == is_dir
    });

    dir_entry.is_some()
}
