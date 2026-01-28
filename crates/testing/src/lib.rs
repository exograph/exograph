// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

pub(crate) mod execution;
pub(crate) mod loader;
mod model;

use std::cmp::min;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use colored::Colorize;

use exo_sql::testing::db::EphemeralDatabaseLauncher;

use model::TestSuite;

pub use execution::get_introspection_result;

/// Loads test files from the supplied directory and runs them using a thread pool.
pub fn run(
    root_directory: &PathBuf,
    pattern: &Option<String>,
    run_introspection_tests: bool,
    generate_rpc_expected: bool,
) -> Result<()> {
    // Make sure deno runtime is initialized in the main thread to work around deno segfault
    // on Linux issue. The tests are run in parallel and will initialize the deno module
    // (and the deno runtime) in child threads, which will cause the crash if we don't do it
    // here first.
    exo_deno::initialize();

    let root_directory_str = root_directory.to_str().unwrap();
    println!(
        "{} {} {} {}",
        "* Running tests in directory".blue().bold(),
        root_directory_str,
        pattern
            .as_ref()
            .map(|p| format!("with pattern '{p}'"))
            .unwrap_or_default(),
        "...".blue().bold(),
    );

    let start_time = std::time::Instant::now();

    let project_tests = TestSuite::load(root_directory, pattern)
        .with_context(|| format!("While loading testfiles from directory {root_directory_str}"))?;
    let number_of_integration_tests = project_tests.len();

    // test introspection for all model files
    if run_introspection_tests {
        println!("{}", "** Introspection tests enabled".blue().bold());
    };

    println!("{}", "** Running integration tests".blue().bold());

    let (tasks, read_tasks) = crossbeam_channel::unbounded::<Box<dyn FnOnce() + Send>>();

    let (tx, rx) = std::sync::mpsc::channel();

    let ephemeral_server = Arc::new(EphemeralDatabaseLauncher::from_env().create_server()?);

    for project_test in project_tests {
        project_test.run(
            run_introspection_tests,
            generate_rpc_expected,
            ephemeral_server.clone(),
            tx.clone(),
            tasks.clone(),
        );
    }

    // Estimate an optimal pool size
    let cpus = num_cpus::get();
    let pool_size = min(number_of_integration_tests, cpus * 2);
    for _ in 0..pool_size {
        let my_reader = read_tasks.clone();
        std::thread::spawn(move || {
            while let Ok(next) = my_reader.recv() {
                next();
            }
        });
    }

    drop(tx);

    let mut test_results = vec![];

    {
        let integration_test_results = rx.into_iter();
        test_results.extend(integration_test_results);
    }

    test_results.sort_by(|a, b| {
        if a.is_err() && b.is_ok() {
            std::cmp::Ordering::Greater
        } else if a.is_ok() && b.is_err() {
            std::cmp::Ordering::Less
        } else if let Ok(a) = a.as_ref() {
            if let Ok(b) = b.as_ref() {
                a.cmp(b)
            } else {
                std::cmp::Ordering::Equal
            }
        } else {
            std::cmp::Ordering::Equal
        }
    });

    let mut number_of_succeeded_tests = 0;

    for result in test_results.iter() {
        match result {
            Ok(result) => {
                println!("{result}");

                if result.is_success() {
                    number_of_succeeded_tests += 1;
                }
            }

            Err(e) => {
                println!("{}", "A testfile errored while running.".red());
                println!("{e:?}\n");
            }
        }
    }

    let number_of_tests = test_results.len();
    let success = number_of_succeeded_tests == number_of_tests;
    let status = if success {
        "PASS.".green()
    } else {
        "FAIL.".red()
    };

    println!(
        "{} {} {} out of {} total in {} seconds ({} cpus)",
        "* Test results:".blue().bold(),
        status,
        format!("{number_of_succeeded_tests} passed").bold(),
        number_of_tests,
        start_time.elapsed().as_secs(),
        cpus,
    );

    if success {
        Ok(())
    } else {
        bail!("Test failures")
    }
}
