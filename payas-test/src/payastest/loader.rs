use serde::Deserialize;
use std::collections::HashMap;
use std::fs::read_dir;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use async_graphql_parser::parse_query;

pub type TestfileSetup = Vec<String>;
pub type TestfileInit = Vec<String>;
pub type TestfileTests = HashMap<String, TestfileTest>;
pub type TestfileTest = Vec<String>;

#[derive(Debug)]
pub enum TestfileOperation {
    Sql(String, String),
    GqlDocument {
        document: String,
        variables: Option<serde_json::Value>,
        expected_payload: Option<serde_json::Value>,
    },
}

#[derive(Debug)]
pub struct ParsedTestfile {
    pub name: String,
    pub unique_dbname: String,

    pub model_path: Option<String>,

    pub init_operations: Vec<TestfileOperation>,
    pub test_operations: HashMap<String, TestfileOperation>,
}

// serde file formats

#[derive(Deserialize, Debug)]
pub struct Testfile {
    pub setup: TestfileSetup,
    pub init: TestfileInit,
    pub tests: TestfileTests,
}

#[derive(Deserialize, Debug)]
pub struct GraphQLFile {
    pub operation: String,
    pub variable: String,
}

/// Load and parse testfiles from a given directory.
pub fn load_testfiles_from_dir(dir: &str) -> Result<Vec<ParsedTestfile>> {
    // enumerate tests
    let path = Path::new(&dir);
    let mut testfiles: Vec<ParsedTestfile> = Vec::new();

    for file in read_dir(&path)? {
        let file = file?;

        if file.path().is_dir() {
            // TODO maybe impose max recursion
            let mut loaded_testfiles = load_testfiles_from_dir(file.path().to_str().unwrap())?;
            testfiles.append(&mut loaded_testfiles);
        }

        if file.path().extension().unwrap_or_default() == "yml" {
            let testfile = load_testfile(&file.path())?;
            testfiles
                .push(parse_testfile(&testfile, &file.path()).context("Failed to parse testfile")?);
        }
    }

    Ok(testfiles)
}

/// Load a specified testfile into memory
fn load_testfile(testfile_path: &Path) -> Result<Testfile> {
    // load test file into memory
    let file = File::open(testfile_path)?;
    let reader = BufReader::new(file);

    let testfile: Testfile = serde_yaml::from_reader(reader)?;
    Ok(testfile)
}

// TODO: handle inline code for all sections
// TODO: handle raw sql for setup and init
/// Parse a deserialized testfile into a data structure.
fn parse_testfile(testfile: &Testfile, testfile_path: &Path) -> Result<ParsedTestfile> {
    let testfile_name = testfile_path
        .file_stem()
        .context("Failed to get file name from path")?
        .to_str()
        .context("Failed to convert file name into Unicode")?
        .to_string();

    let mut result = ParsedTestfile {
        name: testfile_name.clone(),
        unique_dbname: format!(
            "payastest_{}",
            testfile_name.replace(|c: char| !c.is_ascii_alphanumeric(), "_")
        ),

        model_path: None,
        init_operations: Vec::new(),
        test_operations: HashMap::new(),
    };

    // parsing the setup section
    // read out schema path
    // TODO: parse entire setup section
    // TODO: check for a sql file to use
    // TODO: check ext
    let mut model_path = testfile_path.to_path_buf();
    model_path.pop(); // get parent dir
    model_path.push(
        testfile
            .setup
            .get(0)
            .context("No items in the setup section")?,
    );
    result.model_path = Some(
        model_path
            .into_os_string()
            .into_string()
            .map_err(|_| anyhow!("Could not parse model path into a valid Unicode string"))?,
    );

    //println!("Model path: {}", &result.model_path.as_ref().unwrap());

    // read in initialization
    for filename in testfile.init.iter() {
        if filename.ends_with(".gql") {
            result
                .init_operations
                .push(construct_gql_operation_from_file(
                    filename,
                    None,
                    testfile_path,
                )?);
        }
    }

    // read in tests
    for (test_name, test) in &testfile.tests {
        let mut gql_filepath: Option<&String> = None;
        let mut json_filepath: Option<&String> = None;

        // read in a singular test
        for path in test.iter() {
            if path.ends_with(".gql") {
                match gql_filepath {
                    Some(_) => {
                        bail!("Cannot have multiple .gql documents in a single test definition")
                    }
                    None => {
                        gql_filepath = Some(path);
                    }
                }
            }

            if path.ends_with(".json") {
                match json_filepath {
                    Some(_) => {
                        bail!("Cannot have multiple .json expected responses in a single test definition")
                    }
                    None => {
                        json_filepath = Some(path);
                    }
                }
            };
        }

        let test_op = construct_gql_operation_from_file(
            gql_filepath.context("Missing GraphQL document")?,
            Some(json_filepath.context("Missing expected .json response")?),
            testfile_path,
        )?;

        result
            .test_operations
            .insert(test_name.to_string(), test_op);
    }

    Ok(result)
}

fn construct_gql_operation_from_file(
    gql_filepath: &str,
    json_filepath: Option<&str>,
    testfile_basedir: &Path,
) -> Result<TestfileOperation> {
    let gql_file = read_file_from_basedir(gql_filepath, testfile_basedir)
        .with_context(|| format!("Could not read .gql file at {}", gql_filepath))?;
    let gql: GraphQLFile =
        serde_yaml::from_str(&gql_file).context("Could not parse .gql file (is it in YAML?)")?;

    // parse expected json
    let expected_payload = match json_filepath {
        Some(json_filepath) => {
            let json_file = read_file_from_basedir(json_filepath, testfile_basedir)
                .with_context(|| format!("Could not read JSON file at {}", json_filepath))?;
            Some(serde_json::from_str(&json_file).context("Provided JSON is not valid")?)
        }
        None => None,
    };

    // verify gql by parsing
    let _gql_document =
        parse_query(&gql.operation).context("Provided GraphQL is not a valid document")?;

    Ok(TestfileOperation::GqlDocument {
        document: gql.operation,
        variables: serde_json::from_str(&gql.variable)
            .context("GraphQL variable section is not valid JSON")?,
        expected_payload,
    })
}

fn read_file_from_basedir(path: &str, basedir: &Path) -> Result<String> {
    let mut file_path = PathBuf::from(basedir.parent().ok_or("").unwrap());
    file_path.push(path);

    // read in file
    //println!("Reading {:?}", file_path);

    let mut file = File::open(file_path)?;
    let mut buffer = String::new();
    file.read_to_string(&mut buffer)?;

    Ok(buffer)
}
