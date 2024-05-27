use std::{
    path::{Path, PathBuf},
    process::Command,
};

use rexpect::{error::Error, session::spawn_command};

fn exo<I>(cwd: impl AsRef<Path>, args: I) -> Command
where
    I: IntoIterator<Item = &'static str>,
{
    let bin = env!("CARGO_BIN_EXE_exo");

    let mut cmd = Command::new(bin);
    cmd.current_dir(cwd).args(args);
    cmd
}

const EXPECTED_SCHEMA: &str = include_str!("todos.sql");

#[test]
fn exo_smoke_tests() -> Result<(), Error> {
    let cargo_tmp_dir = env!("CARGO_TARGET_TMPDIR");
    let tmp_dir = tempfile::tempdir_in(cargo_tmp_dir).expect("Failed to create tempdir");
    assert!(tmp_dir.path().exists(), "tempdir not found");

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

    assert_eq!(sql.trim(), EXPECTED_SCHEMA.trim());

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
    assert!(path.is_dir(), "Exo project directory {path:?} wasn't found");
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
