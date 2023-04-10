use std::path::PathBuf;

use super::command::{get, get_required, CommandDefinition};
use anyhow::Result;
use clap::{Arg, ArgMatches, Command};

pub struct TestCommandDefinition {}

impl CommandDefinition for TestCommandDefinition {
    fn command(&self) -> Command {
        Command::new("test")
            .about("Perform integration tests")
            .arg(
                Arg::new("dir")
                    .help("The directory containing integration tests.")
                    .value_parser(clap::value_parser!(PathBuf))
                    .required(true)
                    .index(1),
            )
            .arg(
                Arg::new("pattern")
                    .help("Glob pattern to choose which tests to run.")
                    .required(false)
                    .index(2),
            )
            .arg(
                Arg::new("run-introspection-tests")
                    .help("When specified, run standard introspection tests on the tests' model files")
                    .required(false)
                    .long("run-introspection-tests").num_args(0)
            )
    }

    fn execute(&self, matches: &ArgMatches) -> Result<()> {
        let dir: PathBuf = get_required(matches, "dir")?;
        let pattern: Option<String> = get(matches, "pattern"); // glob pattern indicating tests to be executed
        let run_introspection_tests = matches.contains_id("run-introspection-tests");

        testing::run(&dir, &pattern, run_introspection_tests)
    }
}
