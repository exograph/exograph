// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::Error;

use colored::Colorize;
use core_resolver::system_resolver::SystemResolver;

use std::{collections::HashMap, fmt, process::Command};

/// Structure to hold open resources associated with a running testfile.
/// When dropped, we will clean them up.
pub(crate) struct TestfileContext {
    pub server: SystemResolver,
    pub jwtsecret: String,
    pub cookies: HashMap<String, String>,
    pub testvariables: HashMap<String, serde_json::Value>,
}

/// The result of running a testfile.
pub enum TestResultKind {
    Success,
    Fail(Error),
    SetupFail(Error),
}

impl Eq for TestResultKind {}

// We use a custom implementation of PartialEq (needed for sorting)
// that disregards the inner Error because they do not implement PartialEq themselves.
impl PartialEq for TestResultKind {
    fn eq(&self, other: &Self) -> bool {
        match self {
            TestResultKind::Success => matches!(other, TestResultKind::Success),
            TestResultKind::Fail(_) => matches!(other, TestResultKind::Fail(_)),
            TestResultKind::SetupFail(_) => matches!(other, TestResultKind::SetupFail(_)),
        }
    }
}

impl PartialOrd for TestResultKind {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(match self {
            TestResultKind::Success => {
                if matches!(other, TestResultKind::Success) {
                    std::cmp::Ordering::Equal
                } else {
                    std::cmp::Ordering::Greater
                }
            }

            _ => {
                if matches!(other, TestResultKind::Success) {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            }
        })
    }
}

// Represents the result of a test.
#[derive(PartialEq, Eq)]
pub struct TestResult {
    pub log_prefix: String,
    pub result: TestResultKind,
}

impl TestResult {
    pub fn is_success(&self) -> bool {
        matches!(self.result, TestResultKind::Success)
    }
}

impl PartialOrd for TestResult {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TestResult {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // If `a` is successful and `b` isn't, mark `a < b`, so that we get all successful tests
        // shown before the failed ones.
        if self.is_success() && !other.is_success() {
            std::cmp::Ordering::Less
        } else if !self.is_success() && other.is_success() {
            std::cmp::Ordering::Greater
        } else {
            // If both are successful or both are failure, compare it by their log_prefix
            // so multiple tests from the same folder are grouped together
            self.log_prefix.cmp(&other.log_prefix)
        }
    }
}

impl fmt::Display for TestResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.result {
            TestResultKind::Success => {
                writeln!(f, "{} {}", self.log_prefix, "PASS".green())
            }
            TestResultKind::Fail(e) => {
                writeln!(f, "{} {}\n{:?}", self.log_prefix, "FAIL".yellow(), e)
            }
            TestResultKind::SetupFail(e) => writeln!(
                f,
                "{} {}\n{:?}",
                self.log_prefix,
                "TEST SETUP FAILED".red(),
                e
            ),
        }
    }
}

pub(crate) fn cmd(binary_name: &str) -> Command {
    // Pick up the current executable path and replace the file with the specified binary
    // This allows us to invoke `target/debug/exo test ...` or `target/release/exo test ...`
    // without updating the PATH env.
    // Thus, for the former invocation if the `binary_name` is `exo-server` the command will become
    // `<full-path-to>/target/debug/exo-server`
    let mut executable =
        std::env::current_exe().expect("Could not retrieve the current executable");
    executable.set_file_name(binary_name);
    Command::new(
        executable
            .to_str()
            .expect("Could not convert executable path to a string"),
    )
}
