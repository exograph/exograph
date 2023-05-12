// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use clap::{Arg, ArgMatches, Command};

use super::build::BuildError;

#[async_trait]
pub trait CommandDefinition {
    fn command(&self) -> Command;

    async fn execute(&self, matches: &ArgMatches) -> Result<()>;
}

pub struct SubcommandDefinition {
    pub name: &'static str,
    pub about: &'static str,
    pub command_definitions: Vec<Box<dyn CommandDefinition + Sync>>,
}

impl SubcommandDefinition {
    pub fn new(
        name: &'static str,
        about: &'static str,
        command_definitions: Vec<Box<dyn CommandDefinition + Sync>>,
    ) -> Self {
        Self {
            name,
            about,
            command_definitions,
        }
    }
}

#[async_trait]
impl CommandDefinition for SubcommandDefinition {
    fn command(&self) -> Command {
        Command::new(self.name)
            .about(self.about)
            .subcommand_required(true)
            .arg_required_else_help(true)
            .disable_help_subcommand(true)
            .subcommands(
                self.command_definitions
                    .iter()
                    .map(|command_definition| command_definition.command()),
            )
    }

    async fn execute(&self, matches: &ArgMatches) -> Result<()> {
        let subcommand = matches.subcommand().unwrap();

        let command_definition = self
            .command_definitions
            .iter()
            .find(|command_definition| command_definition.command().get_name() == subcommand.0);

        match command_definition {
            Some(command_definition) => command_definition.execute(subcommand.1).await,
            None => Err(anyhow!("Unknown subcommand: {}", subcommand.0)),
        }
    }
}

pub fn get_required<T: Clone + Send + Sync + 'static>(
    matches: &ArgMatches,
    arg_id: &str,
) -> Result<T> {
    get(matches, arg_id).ok_or_else(|| anyhow!("Required argument `{}` is not present", arg_id))
}

pub fn get<T: Clone + Send + Sync + 'static>(matches: &ArgMatches, arg_id: &str) -> Option<T> {
    matches.get_one::<T>(arg_id).cloned()
}

const DEFAULT_MODEL_FILE: &str = "src/index.exo";

pub(crate) fn default_model_file() -> PathBuf {
    PathBuf::from(DEFAULT_MODEL_FILE)
}

pub(crate) fn ensure_exo_project_dir(dir: &Path) -> Result<(), BuildError> {
    if dir.join(default_model_file()).exists() {
        Ok(())
    } else {
        Err(BuildError::UnrecoverableError(anyhow!(
            "The path '{}' does not appear to be an Exograph project. The project directory must include 'src/index.exo'",
            dir.display()
        )))
    }
}

pub fn new_project_arg() -> Arg {
    Arg::new("path")
        .help("Create a new project")
        .long_help("Create a new project in the given path.")
        .required(true)
        .value_parser(clap::value_parser!(PathBuf))
        .index(1)
}

pub fn database_arg() -> Arg {
    Arg::new("database")
        .help("The PostgreSQL database connection string to use. If not specified, the program will attempt to read it from the environment (`EXO_POSTGRES_URL`).")
        .long("database")
        .required(false)
}

pub fn output_arg() -> Arg {
    Arg::new("output")
        .help("Output file path")
        .help("If specified, the output will be written to this file path instead of stdout.")
        .short('o')
        .long("output")
        .required(false)
        .value_parser(clap::value_parser!(PathBuf))
        .num_args(1)
}

pub fn port_arg() -> Arg {
    Arg::new("port")
        .help("Listen port")
        .long_help("The port the server should listen for HTTP requests on.")
        .short('p')
        .long("port")
        .required(false)
        .value_parser(clap::value_parser!(u32))
        .num_args(1)
}
