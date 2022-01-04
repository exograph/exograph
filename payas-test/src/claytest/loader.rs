use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use async_graphql_parser::parse_query;

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum TestfileOperation {
    Sql(String),
    GqlDocument {
        document: String,
        variables: Option<serde_json::Value>,
        expected_payload: Option<serde_json::Value>,
        auth: Option<serde_json::Value>,
    },
}

#[derive(Debug, Default, Clone)]
pub struct ParsedTestfile {
    model_path: PathBuf,
    testfile_path: PathBuf,

    pub init_operations: Vec<TestfileOperation>,
    pub test_operation: Option<TestfileOperation>,
}

impl ParsedTestfile {
    pub fn model_path_string(&self) -> String {
        self.model_path
            .to_str()
            .expect("Failed to convert file name into Unicode")
            .to_string()
    }

    pub fn name(&self) -> String {
        // Setting the extension to "", removes the extension
        Path::with_extension(&self.testfile_path, "")
            .to_str()
            .expect("Failed to convert file name into Unicode")
            .to_string()
    }

    pub fn dbname(&self, dev_model: bool) -> String {
        format!(
            "{}_{}",
            to_postgres(&self.name()),
            if dev_model { "dev" } else { "prod" }
        )
    }
}

// serde file formats

#[derive(Deserialize, Debug)]
pub struct Testfile {
    pub operation: String,
    pub variable: Option<String>,
    pub auth: Option<String>,
    pub response: String,
}

#[derive(Deserialize, Debug)]
pub struct InitFile {
    pub operation: String,
    pub variable: Option<String>,
    pub auth: Option<String>,
}

/// Load and parse testfiles from a given directory.
pub fn load_testfiles_from_dir(path: &Path) -> Result<Vec<ParsedTestfile>> {
    load_testfiles_from_dir_(path, None, &[])
}

fn load_testfiles_from_dir_(
    path: &Path,
    model_path: Option<&Path>,
    init_ops: &[TestfileOperation],
) -> Result<Vec<ParsedTestfile>> {
    let path = PathBuf::from(path);

    // Begin directory traversal
    let mut new_model: Option<PathBuf> = None;
    let mut claytest_files: Vec<PathBuf> = vec![];
    let mut init_files: Vec<PathBuf> = vec![];
    let mut directories: Vec<PathBuf> = vec![];

    for dir_entry in (path.read_dir()?).flatten() {
        if dir_entry.path().is_file() {
            if let Some(extension) = dir_entry.path().extension() {
                // looking for a .clay file in our current directory
                if extension == "clay" {
                    // new .clay file found, use it as our new model
                    if new_model.is_some() {
                        bail!(
                            "Only one .clay file can exist in a directory! Multiple found in {}",
                            path.to_str().unwrap()
                        )
                    }

                    new_model = Some(dir_entry.path())
                }

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

    let model_path = if let Some(new_model) = new_model {
        // use the .clay file we found
        new_model
    } else if let Some(old_model) = model_path {
        // use the previous .clay file
        PathBuf::from(old_model)
    } else {
        // no .clay file found!
        if directories.is_empty() {
            bail!("No model found in {}", path.to_str().unwrap())
        } else {
            // recurse and try to find one
            let mut parsed = vec![];
            for directory in directories {
                let parsed_testfiles = load_testfiles_from_dir_(&directory, None, init_ops)?;
                parsed.extend(parsed_testfiles);
            }

            return Ok(parsed);
        }
    };

    // Parse init files and populate init_ops
    let mut init_ops = init_ops.to_owned();

    for initfile_path in init_files.iter() {
        let init_op = construct_operation_from_init_file(initfile_path)?;
        init_ops.push(init_op);
    }

    // Parse test files
    let mut testfiles = vec![];

    for testfile_path in claytest_files.iter() {
        let testfile = parse_testfile(testfile_path, &model_path.to_path_buf(), init_ops.clone())?;

        testfiles.push(testfile);
    }

    // Recursively parse test files
    for directory in directories.iter() {
        let child_init_ops = init_ops.clone();
        let child_testfiles =
            load_testfiles_from_dir_(directory, Some(&model_path), &child_init_ops)?;
        testfiles.extend(child_testfiles)
    }

    Ok(testfiles)
}

fn parse_testfile(
    testfile_path: &Path,
    model_path: &Path,
    init_ops: Vec<TestfileOperation>,
) -> Result<ParsedTestfile> {
    let file = File::open(testfile_path).context("Could not open test file")?;
    let reader = BufReader::new(file);
    let deserialized_testfile: Testfile = serde_yaml::from_reader(reader)
        .context(format!("Failed to parse test file at {:?}", testfile_path))?;

    // validate GraphQL
    let _gql_document = parse_query(&deserialized_testfile.operation).context("Invalid GraphQL")?;

    Ok(ParsedTestfile {
        model_path: model_path.to_path_buf(),
        testfile_path: testfile_path.to_path_buf(),
        init_operations: init_ops,
        test_operation: Some(TestfileOperation::GqlDocument {
            document: deserialized_testfile.operation.clone(),
            auth: deserialized_testfile.auth.map(from_json).transpose()?,
            variables: deserialized_testfile.variable.map(from_json).transpose()?,
            expected_payload: Some(from_json(deserialized_testfile.response)?),
        }),
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

            Ok(TestfileOperation::GqlDocument {
                document: deserialized_initfile.operation.clone(),
                auth: deserialized_initfile.auth.map(from_json).transpose()?,
                variables: deserialized_initfile.variable.map(from_json).transpose()?,
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
