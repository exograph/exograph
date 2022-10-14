mod claytest;

use anyhow::{bail, Context, Result};
use claytest::integration_tests::{build_claypot_file, run_testfile};
use claytest::loader::{load_testfiles_from_dir, ParsedTestfile};
use rayon::ThreadPoolBuilder;
use std::cmp::min;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use crate::claytest::introspection_tests::run_introspection_test;

/// Loads test files from the supplied directory and runs them using a thread pool.
pub fn run(
    root_directory: &Path,
    pattern: &Option<String>,
    run_introspection_tests: bool,
) -> Result<()> {
    let root_directory_str = root_directory.to_str().unwrap();

    println!(
        "{} {} {} {}",
        ansi_term::Color::Blue
            .bold()
            .paint("* Running tests in directory"),
        root_directory_str,
        pattern
            .as_ref()
            .map(|p| format!("'with pattern {}'", p))
            .unwrap_or_else(|| "".to_string()),
        ansi_term::Color::Blue.bold().paint("..."),
    );

    let start_time = std::time::Instant::now();
    let cpus = num_cpus::get();

    let database_url =
        std::env::var("CLAY_TEST_DATABASE_URL").expect("CLAY_TEST_DATABASE_URL must be specified");

    let testfiles = load_testfiles_from_dir(root_directory, pattern).with_context(|| {
        format!(
            "While loading testfiles from directory {}",
            root_directory_str
        )
    })?;
    let number_of_integration_tests = testfiles.len();

    // Work out which tests share a common clay file so we only build it once for all the
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
        println!(
            "{}",
            ansi_term::Color::Blue
                .bold()
                .paint("** Introspection tests enabled, running")
        );

        for model_path in model_file_deps.keys() {
            test_results.push(run_introspection_test(model_path));
        }
    };

    println!(
        "{}",
        ansi_term::Color::Blue
            .bold()
            .paint("** Running integration tests")
    );

    // Estimate an optimal pool size
    let pool_size = min(number_of_integration_tests, cpus * 2);
    let pool = ThreadPoolBuilder::new()
        .num_threads(pool_size)
        .build()
        .unwrap();

    let (tx, rx) = mpsc::channel();

    // Then build all the model files, spawning the production mode tests once the build completes
    for (model_path, testfiles) in model_file_deps {
        let model_path = model_path.clone();
        let tx = tx.clone();
        let url = database_url.clone();

        pool.spawn(move || match build_claypot_file(&model_path) {
            Ok(()) => {
                for file in testfiles.iter() {
                    let result = run_testfile(file, url.clone());
                    tx.send(result).unwrap();
                }
            }
            Err(e) => tx
                .send(Err(e).with_context(|| {
                    format!(
                        "While trying to build claypot file for {}",
                        model_path.display()
                    )
                }))
                .unwrap(),
        });
    }

    drop(tx);

    let mut integration_test_results: Vec<_> = rx.into_iter().collect();
    integration_test_results.sort_by(|a, b| {
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
    test_results.extend(integration_test_results.into_iter());

    let mut number_of_succeeded_tests = 0;

    for result in test_results.iter() {
        match result {
            Ok(result) => {
                println!("{}", result);

                if result.is_success() {
                    number_of_succeeded_tests += 1;
                }
            }

            Err(e) => {
                println!(
                    "{}",
                    ansi_term::Color::Red.paint("A testfile errored while running.")
                );
                println!(
                    "{}\n",
                    ansi_term::Color::Fixed(240).paint(format!("{:?}", e))
                );
            }
        }
    }

    let number_of_tests = test_results.len();
    let success = number_of_succeeded_tests == number_of_tests;
    let status = if success {
        ansi_term::Color::Green.paint("PASS.")
    } else {
        ansi_term::Color::Red.paint("FAIL.")
    };

    println!(
        "{} {} {} out of {} total in {} seconds ({} cpus)",
        ansi_term::Color::Blue.bold().paint("* Test results:"),
        status,
        ansi_term::Style::new()
            .bold()
            .paint(format!("{} passed", number_of_succeeded_tests)),
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
