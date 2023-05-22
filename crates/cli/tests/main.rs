use std::{
    path::{Path, PathBuf},
    process::Command,
};

use rexpect::{error::Error, session::spawn_command};

#[cfg(debug_assertions)]
const PROFILE: &str = "debug";

#[cfg(not(debug_assertions))]
const PROFILE: &str = "release";

// Create the exo command based on the profile.
// We need the full path to the binary since we will be changing working
// directory during the tests and a relative path from the project directory
// won't be valid.
fn exo<I>(cwd: impl AsRef<Path>, args: I) -> Command
where
    I: IntoIterator<Item = &'static str>,
{
    let exotech_dir = env!("CARGO_MANIFEST_DIR").trim_end_matches("crates/cli");
    let exo = format!("{exotech_dir}/target/{PROFILE}/exo");

    let mut cmd = Command::new(exo);
    cmd.current_dir(cwd).args(args);
    cmd
}

const EXPECTED_SCHEMA: &str = include_str!("todos.sql");

#[test]
fn exo_smoke_tests() -> Result<(), Error> {
    let tmp_dir = tempfile::tempdir().expect("Failed to create tempdir");

    let mut cmd = exo(tmp_dir.path(), ["new", "mariposas"]);
    let p = spawn_command(cmd, Some(5000))?;
    p.process.wait()?;

    let mut project_dir = PathBuf::from(tmp_dir.path());
    project_dir.push("mariposas");
    assert_project_dir(project_dir.clone());

    cmd = exo(project_dir.clone(), ["schema", "create"]);
    let mut p = spawn_command(cmd, Some(5000))?;
    let sql = p.exp_eof()?;
    let sql = sql.replace('\r', "");

    assert_eq!(sql, EXPECTED_SCHEMA);

    cmd = exo(project_dir.clone(), ["build"]);
    let p = spawn_command(cmd, Some(5000))?;
    p.process.wait()?;

    let mut target_dir = project_dir.clone();
    target_dir.push("target");
    assert!(
        target_dir.is_dir(),
        "target directory wasn't found after build"
    );
    let mut ir_file = target_dir.clone();
    ir_file.push("index.exo_ir");
    assert!(ir_file.is_file(), "No exo_ir file found after build");

    Ok(())
}

fn assert_project_dir(mut path: PathBuf) {
    assert!(
        path.is_dir(),
        "Exo project directory {:?} wasn't found",
        path
    );
    let mut tests = path.clone();
    path.push("src");
    assert!(path.is_dir(), "No src directory found");
    path.push("index.exo");
    assert!(path.is_file(), "No index.exo file found");
    tests.push("tests");
    assert!(tests.is_dir(), "No tests directory found");
    let mut gql_file = PathBuf::from(&tests);
    gql_file.push("init.gql");
    assert!(gql_file.is_file(), "No init.gql file found");
    let mut test_file = PathBuf::from(&tests);
    test_file.push("basic-query.exotest");
    assert!(test_file.is_file(), "No exotest file found");
}
