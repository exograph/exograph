use std::path::PathBuf;

use super::command::{get, get_required, CommandDefinition};
use anyhow::{anyhow, Result};
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
    }

    fn execute(&self, matches: &ArgMatches) -> Result<()> {
        let dir: PathBuf = get_required(matches, "dir")?;
        let pattern: Option<String> = get(matches, "pattern"); // glob pattern indicating tests to be executed

        let run_introspection_tests: bool = match std::env::var("EXO_RUN_INTROSPECTION_TESTS") {
            Ok(e) => match e.to_lowercase().as_str() {
                "true" | "1" => Ok(true), // The standard convention for boolean env vars is to accept "1" as true, as well
                "false" => Ok(false),
                _ => Err(anyhow!(
                    "EXO_RUN_INTROSPECTION_TESTS env var must be set to a boolean or 1",
                )),
            },
            Err(_) => Ok(false),
        }?;

        testing::run(&dir, &pattern, run_introspection_tests)
    }
}
