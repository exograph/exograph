use serde::Deserialize;
use std::ffi::OsStr;
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
        auth: Option<serde_json::Value>,
    },
}

#[derive(Debug, Clone)]
pub struct ParsedTestfile {
    root_directory: PathBuf, // Root directory specified when invoking `clay test <root_directory>
    model_path: PathBuf,
    testfile_path: PathBuf,

    pub init_operations: Vec<TestfileOperation>,
    pub test_operation_stages: Vec<TestfileOperation>,
}

impl ParsedTestfile {
    pub fn model_path_string(&self) -> String {
        self.model_path
            .canonicalize()
            .expect(&format!(
                "Failed to canonicalize model path {}",
                self.model_path.to_string_lossy()
            ))
            .to_str()
            .expect("Failed to convert file name into Unicode")
            .to_string()
    }

    pub fn name(&self) -> String {
        let relative_testfile_path = &self
            .testfile_path
            .strip_prefix(
                self.root_directory
                    .to_str()
                    .expect("Could not get string for the root directory"),
            )
            .expect("Failed to obtain relative path to testfile");

        // Drop to extension (".claytest") to obtain the name
        relative_testfile_path
            .with_extension("")
            .to_str()
            .expect("Failed to convert file name into Unicode")
            .to_string()
    }

    pub fn dbname(&self) -> String {
        to_postgres(&self.name())
    }
}

// serde file formats

#[derive(Deserialize, Debug, Clone)]
pub struct TestfileStage {
    pub clayfile: Option<String>,
    pub operation: String,
    pub variable: Option<String>,
    pub auth: Option<String>,
    pub response: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct TestfileMultipleStages {
    pub clayfile: Option<String>,
    pub stages: Vec<TestfileStage>,
}

#[derive(Deserialize, Debug)]
pub struct InitFile {
    pub operation: String,
    pub variable: Option<String>,
    pub auth: Option<String>,
}

/// Load and parse testfiles from a given directory.
pub fn load_testfiles_from_dir(
    root_directory: &Path,
    pattern: &Option<String>,
) -> Result<Vec<ParsedTestfile>> {
    load_testfiles_from_dir_(root_directory, root_directory, &[], pattern)
}

fn load_testfiles_from_dir_(
    root_directory: &Path,
    directory: &Path,
    init_ops: &[TestfileOperation],
    pattern: &Option<String>,
) -> Result<Vec<ParsedTestfile>> {
    let directory = PathBuf::from(directory);

    // Begin directory traversal
    let mut claytest_files: Vec<PathBuf> = vec![];
    let mut init_files: Vec<PathBuf> = vec![];
    let mut directories: Vec<PathBuf> = vec![];

    for dir_entry in (directory.read_dir()?).flatten() {
        if dir_entry.path().is_file() {
            if let Some(extension) = dir_entry.path().extension() {
                // looking for .claytest files in our current directory
                if extension == "claytest" {
                    claytest_files.push(dir_entry.path());
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
            directories.push(dir_entry.path())
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

    for testfile_path in claytest_files.iter() {
        let testfile = parse_testfile(root_directory, testfile_path, init_ops.clone())?;

        testfiles.push(testfile);
    }

    // Recursively parse test files
    for directory in directories.iter() {
        let child_init_ops = init_ops.clone();
        let child_testfiles =
            load_testfiles_from_dir_(root_directory, directory, &child_init_ops, pattern)?;
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
    root_directory: &Path,
    testfile_path: &Path,
    init_ops: Vec<TestfileOperation>,
) -> Result<ParsedTestfile> {
    let mut file = File::open(testfile_path).context("Could not open test file")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .context("Could not read test file to string")?;

    let deserialized_testfile_multiple_stages: Result<TestfileMultipleStages, _> =
        serde_yaml::from_str(&contents);
    let deserialized_testfile_single_stage: Result<TestfileStage, _> =
        serde_yaml::from_str(&contents);

    let (stages, clayfile_path) = if let Ok(testfile) = deserialized_testfile_multiple_stages {
        (testfile.stages, testfile.clayfile)
    } else if let Ok(stage) = deserialized_testfile_single_stage {
        (vec![stage.clone()], stage.clayfile)
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

    let testfile_folder = testfile_path.parent().expect("Testfile has no parent?");
    let model_path = if let Some(path) = clayfile_path {
        // test specifies a root clayfile, use that
        testfile_folder.to_owned().join(path)
    } else {
        // see if the testfile's directory has a single clay file we can use
        let clay_files = std::fs::read_dir(testfile_folder)?
            .collect::<Result<Vec<_>, std::io::Error>>()?
            .into_iter()
            .filter(|dir| matches!(dir.path().extension().and_then(OsStr::to_str), Some("clay")))
            .collect::<Vec<_>>();

        if clay_files.len() == 1 {
            clay_files[0].path()
        } else if clay_files.len() > 1 {
            bail!(
                "Multiple .clay files found for {}, please manually specify a root model file",
                testfile_path.to_string_lossy()
            )
        } else {
            bail!(
                "No .clay file specified nor found in {} for testfile {}",
                testfile_folder.to_string_lossy(),
                testfile_path.to_string_lossy()
            )
        }
    };

    // validate GraphQL
    let test_operation_sequence = stages
        .into_iter()
        .map(|stage| {
            let gql_document = parse_query(&stage.operation).context("Invalid GraphQL")?;

            Ok(TestfileOperation::GqlDocument {
                document: stage.operation,
                testvariable_bindings: build_testvariable_bindings(&gql_document),
                auth: stage.auth.map(from_json).transpose()?,
                variables: stage.variable,
                expected_payload: stage.response,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(ParsedTestfile {
        root_directory: root_directory.to_path_buf(),
        model_path: model_path.to_path_buf(),
        testfile_path: testfile_path.to_path_buf(),
        init_operations: init_ops,
        test_operation_stages: test_operation_sequence,
    })
}

fn construct_operation_from_init_file(path: &Path) -> Result<TestfileOperation> {
    match path.extension().unwrap().to_str().unwrap() {
        "sql" => {
            let sql = std::fs::read_to_string(&path).context("Failed to read SQL file")?;

            Ok(TestfileOperation::Sql(sql))
        }
        "gql" => {
            let file = File::open(path)?;
            let reader = BufReader::new(file);
            let deserialized_initfile: InitFile =
                serde_yaml::from_reader(reader).context(format!("Failed to parse {:?}", path))?;

            // validate GraphQL
            let gql_document =
                parse_query(&deserialized_initfile.operation).context("Invalid GraphQL")?;

            Ok(TestfileOperation::GqlDocument {
                document: deserialized_initfile.operation.clone(),
                testvariable_bindings: build_testvariable_bindings(&gql_document),
                auth: deserialized_initfile.auth.map(from_json).transpose()?,
                variables: deserialized_initfile.variable,
                expected_payload: None,
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

// Generate a unique, PostgreSQL-friendly name from a `str`.
fn to_postgres(name: &str) -> String {
    format!("claytest_{:x}", md5::compute(name))
}
