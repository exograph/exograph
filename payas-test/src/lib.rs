mod claytest;

use anyhow::{bail, Result};
use claytest::loader::load_testfiles_from_dir;
use claytest::runner::run_testfile;
use std::path::Path;

pub fn run(directory: &Path) -> Result<()> {
    println!(
        "{} {} {}",
        ansi_term::Color::Blue
            .bold()
            .paint("* Running tests in directory"),
        directory.to_str().unwrap(),
        ansi_term::Color::Blue.bold().paint("..."),
    );

    // Load testfiles
    let testfiles = load_testfiles_from_dir(Path::new(&directory)).unwrap();
    let number_of_tests = testfiles.len() * 2; // *2 because we run each testfile twice: dev mode and qproduction mode

    // Run testfiles in parallel
    let mut test_results: Vec<_> = testfiles
        .into_iter()
        .flat_map(|t| {
            let t_dev = t.clone();
            vec![
                std::thread::spawn(move || {
                    run_testfile(
                        &t_dev,
                        std::env::var("CLAY_TEST_DATABASE_URL").unwrap(),
                        true, // dev_mode
                    )
                }),
                std::thread::spawn(move || {
                    run_testfile(&t, std::env::var("CLAY_TEST_DATABASE_URL").unwrap(), false)
                }),
            ]
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|j| j.join().unwrap())
        .collect();

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
                result.as_ref().unwrap();
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
        "{} {} {} out of {} total",
        ansi_term::Color::Blue.bold().paint("* Test results:"),
        status,
        ansi_term::Style::new()
            .bold()
            .paint(format!("{} passed", number_of_succeeded_tests)),
        number_of_tests
    );

    if success {
        Ok(())
    } else {
        bail!("Test failures")
    }
}
