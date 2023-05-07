// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

mod exotest;

use anyhow::{bail, Context, Result};
use colored::Colorize;

use exo_sql::testing::db::{EphemeralDatabaseLauncher, EphemeralDatabaseServer};
use exotest::integration_tests::{build_exo_ir_file, run_testfile};
use exotest::loader::{load_testfiles_from_dir, ParsedTestfile};
use rayon::ThreadPoolBuilder;
use std::cmp::min;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{mpsc, Arc};

use crate::exotest::introspection_tests::run_introspection_test;

/// Loads test files from the supplied directory and runs them using a thread pool.
pub fn run(
    root_directory: &PathBuf,
    pattern: &Option<String>,
    run_introspection_tests: bool,
) -> Result<()> {
    let root_directory_str = root_directory.to_str().unwrap();

    println!(
        "{} {} {} {}",
        "* Running tests in directory".blue().bold(),
        root_directory_str,
        pattern
            .as_ref()
            .map(|p| format!("with pattern '{p}'"))
            .unwrap_or_else(|| "".to_string()),
        "...".blue().bold(),
    );

    let start_time = std::time::Instant::now();
    let cpus = num_cpus::get();

    let testfiles = load_testfiles_from_dir(root_directory, pattern)
        .with_context(|| format!("While loading testfiles from directory {root_directory_str}"))?;
    let number_of_integration_tests = testfiles.len();

    // Work out which tests share a common exo file so we only build it once for all the
    // dependent tests, avoiding accidental corruption from overwriting.
    let mut model_file_deps: HashMap<PathBuf, Vec<ParsedTestfile>> = HashMap::new();

    for f in testfiles.into_iter() {
        if let Some(files) = model_file_deps.get_mut(&f.model_path) {
            files.push(f);
        } else {
            model_file_deps.insert(f.model_path.clone(), vec![f]);
        }
    }

    let mut test_results = vec![];

    // test introspection for all model files
    if run_introspection_tests {
        println!("{}", "** Introspection tests enabled".blue().bold());
    };

    println!("{}", "** Running integration tests".blue().bold());

    // Estimate an optimal pool size
    let pool_size = min(number_of_integration_tests, cpus * 2);
    let pool = ThreadPoolBuilder::new()
        .num_threads(pool_size)
        .build()
        .unwrap();

    let (tx, rx) = mpsc::channel();

    let ephemeral_server = Arc::new(EphemeralDatabaseLauncher::create_server()?);

    // Then build all the model files, spawning the production mode tests once the build completes
    for (model_path, testfiles) in model_file_deps {
        let model_path = model_path.clone();
        let tx = tx.clone();
        let ephemeral_server = ephemeral_server.clone();

        pool.spawn(move || match build_exo_ir_file(&model_path) {
            Ok(()) => {
                if run_introspection_tests {
                    tx.send(run_introspection_test(&model_path)).unwrap();
                };

                for file in testfiles.iter() {
                    let result = run_testfile(
                        file,
                        ephemeral_server.as_ref().as_ref() as &dyn EphemeralDatabaseServer,
                    );
                    tx.send(result).unwrap();
                }
            }
            Err(e) => tx
                .send(Err(e).with_context(|| {
                    format!(
                        "While trying to build exo_ir file for {}",
                        model_path.display()
                    )
                }))
                .unwrap(),
        });
    }

    drop(tx);

    {
        let integration_test_results = rx.into_iter();
        test_results.extend(integration_test_results.into_iter());
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
