// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{path::PathBuf, sync::Arc};

use super::command::{CommandDefinition, get, get_required};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use clap::{Arg, ArgMatches, Command};
use common::env_const::EXO_ENV;
use exo_env::Environment;
use exo_sql::testing::db::EXO_SQL_EPHEMERAL_DATABASE_LAUNCH_PREFERENCE;

use crate::config::Config;

const EXO_RUN_INTROSPECTION_TESTS: &str = "EXO_RUN_INTROSPECTION_TESTS";
const EXO_RPC_GENERATE_EXPECTED: &str = "EXO_RPC_GENERATE_EXPECTED";

pub struct TestCommandDefinition {}

#[async_trait]
impl CommandDefinition for TestCommandDefinition {
    fn command(&self) -> Command {
        Command::new("test")
            .about("Perform integration tests")
            .arg(
                Arg::new("dir")
                    .help("The directory containing integration tests.")
                    .default_value(".")
                    .value_parser(clap::value_parser!(PathBuf))
                    .required(false)
                    .index(1),
            )
            .arg(
                Arg::new("pattern")
                    .help("Glob pattern to choose which tests to run.")
                    .required(false)
                    .index(2),
            )
    }

    async fn execute(
        &self,
        matches: &ArgMatches,
        _config: &Config,
        _env: Arc<dyn Environment>,
    ) -> Result<()> {
        let dir: PathBuf = get_required(matches, "dir")?;
        let pattern: Option<String> = get(matches, "pattern"); // glob pattern indicating tests to be executed

        let run_introspection_tests: bool = match std::env::var(EXO_RUN_INTROSPECTION_TESTS) {
            Ok(e) => match e.to_lowercase().as_str() {
                "true" | "1" => Ok(true), // The standard convention for boolean env vars is to accept "1" as true, as well
                "false" => Ok(false),
                _ => Err(anyhow!(
                    "{EXO_RUN_INTROSPECTION_TESTS} env var must be set to a boolean or 1",
                )),
            },
            Err(_) => Ok(false),
        }?;

        let generate_rpc_expected: bool = std::env::var(EXO_RPC_GENERATE_EXPECTED).is_ok();

        // Clear all EXO_ env vars before running tests (this way, if the user has set any, they won't affect the tests)
        for (key, _) in std::env::vars() {
            if key.starts_with("EXO_") && key != EXO_SQL_EPHEMERAL_DATABASE_LAUNCH_PREFERENCE {
                // SAFETY: std::env::remove_var is unsafe if called from multiple threads. This is a start of the process and still in the main thread.
                unsafe {
                    std::env::remove_var(key);
                }
            }
        }

        // SAFETY: std::env::set_var is unsafe if called from multiple threads. This is a start of the process and still in the main thread.
        unsafe {
            std::env::set_var(EXO_ENV, "test");
        }

        testing::run(
            &dir,
            &pattern,
            run_introspection_tests,
            generate_rpc_expected,
        )
    }
}
