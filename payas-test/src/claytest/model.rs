use crate::claytest::dbutils::dropdb_psql;
use anyhow::Error;
use std::{fmt, process::Child};

/// Structure to hold open resources associated with a running testfile.
/// When dropped, we will clean them up.
#[derive(Default)]
pub struct TestfileContext {
    pub dbname: Option<String>,
    pub dburl: Option<String>,
    pub server: Option<Child>,
}

impl Drop for TestfileContext {
    fn drop(&mut self) {
        // kill the started server
        if let Some(server) = &mut self.server {
            if let e @ Err(_) = server.kill() {
                println!("Error killing server instance: {:?}", e)
            }
        }

        // drop the database
        if let Some(dburl) = &self.dburl {
            if let Some(dbname) = &self.dbname {
                if let e @ Err(_) = dropdb_psql(dbname, dburl) {
                    println!("Error dropping {} using {}: {:?}", dbname, dburl, e)
                }
            }
        }
    }
}

/// The result of running a testfile.
pub enum TestResult {
    Success,
    AssertionFail(Error),
    SetupFail(Error),
}

impl Eq for TestResult {}

// We use a custom implementation of PartialEq (needed for sorting)
// that disregards the inner Error because they do not implement PartialEq themselves.
impl PartialEq for TestResult {
    fn eq(&self, other: &Self) -> bool {
        match self {
            TestResult::Success => matches!(other, TestResult::Success),
            TestResult::AssertionFail(_) => matches!(other, TestResult::AssertionFail(_)),
            TestResult::SetupFail(_) => matches!(other, TestResult::SetupFail(_)),
        }
    }
}

impl PartialOrd for TestResult {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(match self {
            TestResult::Success => {
                if matches!(other, TestResult::Success) {
                    std::cmp::Ordering::Equal
                } else {
                    std::cmp::Ordering::Greater
                }
            }

            _ => {
                if matches!(other, TestResult::Success) {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            }
        })
    }
}

/// The collected output (`stdout`, `stderr`) from a instance of `clay`.
#[derive(PartialEq, Eq)]
pub struct TestOutput {
    pub log_prefix: String,
    pub result: TestResult,
    pub output: String,
}

impl TestOutput {
    pub fn is_success(&self) -> bool {
        matches!(self.result, TestResult::Success)
    }
}

impl PartialOrd for TestOutput {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TestOutput {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.is_success() && !other.is_success() {
            std::cmp::Ordering::Greater
        } else if !self.is_success() && other.is_success() {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Equal
        }
    }
}

impl fmt::Display for TestOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.result {
            TestResult::Success => {
                writeln!(
                    f,
                    "{} {}",
                    self.log_prefix,
                    ansi_term::Color::Green.paint("OK")
                )
            }
            TestResult::AssertionFail(e) => writeln!(
                f,
                "{} {}\n{:?}",
                self.log_prefix,
                ansi_term::Color::Yellow.paint("ASSERTION FAILED"),
                e
            ),
            TestResult::SetupFail(e) => writeln!(
                f,
                "{} {}\n{:?}",
                self.log_prefix,
                ansi_term::Color::Red.paint("TEST SETUP FAILED"),
                e
            ),
        }
        .unwrap();

        if !matches!(&self.result, TestResult::Success) {
            write!(
                f,
                "{}\n{}\n",
                ansi_term::Color::Purple.paint(":: Output:"),
                ansi_term::Color::Fixed(240).paint(&self.output)
            )
        } else {
            Ok(())
        }
    }
}
