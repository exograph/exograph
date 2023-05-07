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

use super::testvariable_bindings::build_testvariable_bindings;
use super::testvariable_bindings::TestvariableBindings;

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum TestfileOperation {
    Sql(String),
    GqlDocument {
        document: String,
        testvariable_bindings: TestvariableBindings,
        variables: Option<String>,        // stringified
        expected_payload: Option<String>, // stringified
        deno_prelude: Option<String>,
        auth: Option<serde_json::Value>,
        headers: Option<String>, // stringified
    },
}

#[derive(Debug, Clone)]
pub struct ParsedTestfile {
    pub model_path: PathBuf,
    testfile_path: PathBuf,
    pub extra_envs: HashMap<String, String>, // extra envvars to set for the entire testfile
    pub init_operations: Vec<TestfileOperation>,
    pub test_operation_stages: Vec<TestfileOperation>,
}

impl ParsedTestfile {
    pub fn model_path_string(&self) -> String {
        self.model_path
            .canonicalize()
            .unwrap_or_else(|_| {
                panic!(
                    "Failed to canonicalize model path {}",
                    self.model_path.to_string_lossy()
                )
            })
            .to_str()
            .expect("Failed to convert file name into Unicode")
            .to_string()
    }

    pub fn name(&self) -> String {
        let relative_testfile_path = &self
            .testfile_path
            .strip_prefix("./")
            .expect("Failed to obtain relative path to testfile");

        // Drop to extension (".exotest") to obtain the name
        relative_testfile_path
            .with_extension("")
            .to_str()
            .expect("Failed to convert file name into Unicode")
            .to_string()
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

/// Load and parse testfiles from a given directory.
pub fn load_testfiles_from_dir(
    root_directory: &PathBuf,
    pattern: &Option<String>,
) -> Result<Vec<ParsedTestfile>> {
    fn is_exoproject(dir: &Path) -> bool {
        directory_contains(dir, "src", true) && {
            let src_dir = dir.join("src");
            directory_contains(&src_dir, "index.exo", false)
        }
    }

    fn has_tests(dir: &Path) -> bool {
        directory_contains(dir, "tests", true)
    }

    if is_exoproject(root_directory) && has_tests(root_directory) {
        return load_testfiles_from_dir_(root_directory, &[], pattern);
    }

    // exo projects that have tests
    let exo_projects = root_directory
        .read_dir()
        .context(format!(
            "Could not read {} directory",
            root_directory.display()
        ))?
        .flatten()
        .filter(|dir_entry| {
            let dir_path = dir_entry.path();
            is_exoproject(&dir_path) && has_tests(&dir_path)
        });

    exo_projects
        .map(|exo_project| {
            let exo_project_directory = exo_project.path();
            load_testfiles_from_dir_(&exo_project_directory, &[], pattern)
        })
        .collect::<Result<Vec<_>>>()
        .map(|tests| tests.into_iter().flatten().collect::<Vec<_>>())
}

fn load_testfiles_from_dir_(
    exo_project_directory: &PathBuf, // directory that contains "src/index.exo"
    init_ops: &[TestfileOperation],
    pattern: &Option<String>,
) -> Result<Vec<ParsedTestfile>> {
    let test_directory = exo_project_directory.join("tests");

    if !Path::exists(&test_directory) {
        return Ok(vec![]);
    }

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
        let testfile = parse_testfile(exo_project_directory, testfile_path, init_ops.clone())?;

        testfiles.push(testfile);
    }

    // Recursively parse test files
    for directory in sub_directories.iter() {
        let child_init_ops = init_ops.clone();
        let child_testfiles = load_testfiles_from_dir_(directory, &child_init_ops, pattern)?;
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
    exo_project_directory: &PathBuf,
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
            let testvariable_bindings = parse_query(&stage.operation)
                .map(|gql_document| build_testvariable_bindings(&gql_document))
                .unwrap_or_else(|_| {
                    eprintln!(
                        "Invalid GraphQL document; defaulting test variables binding to empty"
                    );
                    HashMap::new()
                });

            Ok(TestfileOperation::GqlDocument {
                document: stage.operation,
                testvariable_bindings,
                auth: stage.auth.map(from_json).transpose()?,
                variables: stage.variable,
                expected_payload: stage.response,
                headers: stage.headers,
                deno_prelude: stage.deno,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(ParsedTestfile {
        model_path: exo_project_directory.to_owned(),
        testfile_path: testfile_path.to_path_buf(),
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
                testvariable_bindings: build_testvariable_bindings(&gql_document),
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

fn directory_contains(dir: &Path, name: &str, is_dir: bool) -> bool {
    if !dir.is_dir() {
        return false;
    }

    let dir_entry = dir.read_dir().unwrap().flatten().find(|dir_entry| {
        dir_entry.file_name() == name && dir_entry.file_type().unwrap().is_dir() == is_dir
    });

    dir_entry.is_some()
}
