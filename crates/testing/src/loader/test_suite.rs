// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::path::Path;
use std::path::PathBuf;
use wildmatch::WildMatch;

use anyhow::Result;

use crate::model::{InitOperation, IntegrationTest, TestSuite};

impl TestSuite {
    /// Load and parse testfiles from a given directory and pattern.
    pub fn load(root_directory: &PathBuf, pattern: &Option<String>) -> Result<Vec<TestSuite>> {
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
                let tests = load_tests_dir(
                    &exo_project_dir,
                    &[],
                    pattern,
                    root_directory,
                    &exo_project_dir,
                )?;
                Ok(TestSuite {
                    project_dir: exo_project_dir,
                    tests,
                })
            })
            .collect::<Result<Vec<_>>>()
    }
}

fn load_tests_dir(
    test_directory: &Path, // directory that contains "src/index.exo"
    init_ops: &[InitOperation],
    pattern: &Option<String>,
    root_dir: &Path,
    project_dir: &Path, // The exo project directory
) -> Result<Vec<IntegrationTest>> {
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
        let init_op = IntegrationTest::load_init_operations(initfile_path)?;
        init_ops.extend(init_op);
    }

    // Parse test files
    let mut testfiles = vec![];

    for testfile_path in exotest_files.iter() {
        let testfile =
            IntegrationTest::load(testfile_path, init_ops.clone(), root_dir, project_dir)?;
        testfiles.push(testfile);
    }

    // Recursively parse test files
    for sub_directory in sub_directories.iter() {
        let child_init_ops = init_ops.clone();
        let child_testfiles = load_tests_dir(
            sub_directory,
            &child_init_ops,
            pattern,
            root_dir,
            project_dir,
        )?;
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
        // An entry may be a symlink, so we canonicalize it to get the actual path (else
        // dir_entry.is_dir() will return false even when the link points to a directory)
        let entry_path = std::fs::canonicalize(dir_entry.path()).unwrap();

        dir_entry.file_name() == name && std::fs::metadata(entry_path).unwrap().is_dir() == is_dir
    });

    dir_entry.is_some()
}
