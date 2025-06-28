// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

#![cfg(all(any(feature = "test-support", test), not(target_family = "wasm")))]

use colored::Colorize;
use std::{
    future::Future,
    path::{Path, PathBuf},
};
use wildmatch::WildMatch;

/// Run tests in a given folder that matches the following structure:
///
/// This function will run the tests in the test-name1, test-name2 folder
pub async fn run_tests<Fut, T, E>(
    crate_root_dir: &'static str,
    filter_env_var: &'static str,
    test_root_dir: &'static str,
    test_fn: impl Fn(String, PathBuf) -> Fut,
) -> Result<(), E>
where
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let filter = std::env::var(filter_env_var).unwrap_or("*".to_string());
    let wildcard = WildMatch::new(&filter);

    let test_configs_dir = relative_path(crate_root_dir, test_root_dir, "", "");
    let test_configs = std::fs::read_dir(test_configs_dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().unwrap().is_dir())
        .filter(|entry| wildcard.matches(entry.file_name().to_str().unwrap()));

    let mut failed_tests = vec![];

    for test_config in test_configs {
        let test_config_name = test_config.file_name();
        let test_config_name = test_config_name.to_str().unwrap();
        let test_path = relative_path(crate_root_dir, test_root_dir, test_config_name, "");
        if let Err(e) = test_fn(test_config_name.to_string(), test_path).await {
            println!("{}: {}", test_config_name, "failed".red());
            println!("{}", e);
            failed_tests.push(test_config_name.to_string());
        }
    }

    if !failed_tests.is_empty() {
        panic!(
            "{} (filter: '{filter_env_var}' set to '{filter}'):\n\t{}",
            "The following tests failed".red(),
            failed_tests.join("\n\t")
        );
    } else {
        Ok(())
    }
}

fn relative_path(
    crate_root_dir: &'static str,
    root_dir: &str,
    folder: &str,
    path: &str,
) -> PathBuf {
    let base_path = Path::new(crate_root_dir).join(root_dir);

    if folder.is_empty() {
        return base_path;
    }

    let folder_path = base_path.join(folder);

    if path.is_empty() {
        return folder_path;
    }

    folder_path.join(path)
}

pub fn read_relative_file(test_path: &PathBuf, path: &str) -> Result<String, std::io::Error> {
    std::fs::read_to_string(test_path.join(path))
}

pub fn assert_file_content(
    test_path: &PathBuf,
    path: &str,
    actual_content: &str,
    test_name: &str,
) -> Result<(), String> {
    let expected_content = read_relative_file(test_path, path).unwrap();
    let expected_content = expected_content.trim();

    let actual_file_name = {
        if path.contains(".expected.") {
            path.replace(".expected.", ".actual.")
        } else {
            // Drop the extension and add ".actual" followed by the extension
            let extension = path.split('.').last().unwrap();
            let file_name = path.split('.').next().unwrap();
            format!("{}.actual.{}", file_name, extension)
        }
    };
    let actual_file = test_path.join(&actual_file_name);

    let actual_content = actual_content.trim();

    if !compare_strings_ignoring_whitespace(actual_content, &expected_content) {
        std::fs::write(actual_file, actual_content).unwrap();
        return Err(format!("{}: {}", "File content mismatch".red(), test_name));
    } else {
        if actual_file.exists() {
            std::fs::remove_file(actual_file).unwrap();
        }
    }

    Ok(())
}

fn compare_strings_ignoring_whitespace(a: &str, b: &str) -> bool {
    let a_lines = a.lines().map(|line| line.trim()).collect::<Vec<_>>();
    let b_lines = b.lines().map(|line| line.trim()).collect::<Vec<_>>();
    a_lines.len() == b_lines.len()
        && a_lines
            .iter()
            .zip(b_lines.iter())
            .all(|(a_line, b_line)| a_line == b_line)
}
