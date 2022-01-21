mod claytest;

use anyhow::{bail, Context, Result};
use claytest::loader::{load_testfiles_from_dir, ParsedTestfile};
use claytest::runner::{build_claypot_file, run_testfile};
use rayon::ThreadPoolBuilder;
use std::cmp::min;
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;

/// Loads test files from the supplied directory and runs them using a thread pool.
pub fn run(directory: &Path, pattern: &Option<String>) -> Result<()> {
    let directory_str = directory.to_str().unwrap();

    println!(
        "{} {} {} {}",
        ansi_term::Color::Blue
            .bold()
            .paint("* Running tests in directory"),
        directory_str,
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

    let testfiles = load_testfiles_from_dir(Path::new(&directory), pattern)
        .with_context(|| format!("While loading testfiles from directory {}", directory_str))?;

    let number_of_tests = testfiles.len();

    // Work out which tests share a common clay file so we only build it once for all the
    // dependent tests, avoiding accidental corruption from overwriting.
    let mut model_file_deps: HashMap<String, Vec<ParsedTestfile>> = HashMap::new();

    for f in testfiles.iter() {
        if let Some(files) = model_file_deps.get_mut(&f.model_path_string()) {
            files.push(f.clone());
        } else {
            model_file_deps.insert(f.model_path_string(), vec![f.clone()]);
        }
    }

    // Estimate an optimal pool size
    let pool_size = min(number_of_tests, cpus * 2);
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
                    format!("While trying to build claypot file for {}", model_path)
                }))
                .unwrap(),
        });
    }

    drop(tx);

    let mut test_results: Vec<_> = rx.into_iter().collect();

    test_results.sort_by(|a, b| {
        if a.is_ok() && b.is_err() {
            std::cmp::Ordering::Greater
        } else if a.is_err() && b.is_ok() {
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
                println!("{}", result);

                if result.is_success() {
                    number_of_succeeded_tests += 1;
                }
            }

            Err(e) => {
                println!("Testfile failure: {:?}", e)
            }
        }
    }

    let success = number_of_succeeded_tests == number_of_tests;
    let status = if success {
        ansi_term::Color::Green.paint("OK.")
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
