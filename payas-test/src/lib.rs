mod claytest;

use anyhow::{bail, Result};
use claytest::loader::{load_testfiles_from_dir, ParsedTestfile};
use claytest::runner::run_testfile;
use rayon::ThreadPoolBuilder;
use std::cmp::min;
use std::path::Path;
use std::sync::mpsc;

/// Loads test files from the supplied directory and runs them using a thread pool.
pub fn run(directory: &Path) -> Result<()> {
    println!(
        "{} {} {}",
        ansi_term::Color::Blue
            .bold()
            .paint("* Running tests in directory"),
        directory.to_str().unwrap(),
        ansi_term::Color::Blue.bold().paint("..."),
    );
    let start_time = std::time::Instant::now();
    let cpus = num_cpus::get();

    let database_url = std::env::var("CLAY_TEST_DATABASE_URL").expect("CLAY_TEST_DATABASE_URL");

    let testfiles = load_testfiles_from_dir(Path::new(&directory)).unwrap();
    let number_of_tests = testfiles.len() * 2; // *2 because we run each testfile twice: dev mode and production mode

    // Estimate an optimal pool size
    let pool_size = min(number_of_tests, cpus * 2);
    let pool = ThreadPoolBuilder::new()
        .num_threads(pool_size)
        .build()
        .unwrap();

    let (tx, rx) = mpsc::channel();

    let run_test_in_pool = |file: ParsedTestfile, is_dev_mode: bool| {
        let tx = tx.clone();
        let url = database_url.clone();

        pool.spawn(move || {
            let result = run_testfile(&file, url, is_dev_mode);
            tx.send(result).unwrap();
        });
    };

    // Run testfiles in parallel using the thread pool in both production and dev modes
    for file in testfiles.iter() {
        run_test_in_pool(file.clone(), false);
        run_test_in_pool(file.clone(), true);
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

    test_results.reverse();

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
