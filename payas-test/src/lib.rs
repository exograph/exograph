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
    let number_of_tests = testfiles.len();

    // Run testfiles in parallel
    let number_of_succeeded_tests = testfiles
        .into_iter()
        .map(|t| {
            std::thread::spawn(move || {
                run_testfile(&t, std::env::var("CLAY_TEST_DATABASE_URL").unwrap())
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|j| j.join().unwrap())
        .fold(0, |accum, test_status| match test_status {
            Ok(test_result) => accum + test_result,
            Err(e) => {
                println!("Testfile failure: {:?}", e);
                accum
            }
        });

    let success = number_of_succeeded_tests == number_of_tests;
    let status = if success {
        ansi_term::Color::Green.paint("OK.")
    } else {
        ansi_term::Color::Red.paint("FAIL.")
    };

    println!(
        "{} {} {} out of {} total",
        ansi_term::Color::Blue.bold().paint("* Test result:"),
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
