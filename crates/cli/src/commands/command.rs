// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use clap::{Arg, ArgMatches, Command};
use colored::Colorize;
use common::env_const::_EXO_ENFORCE_TRUSTED_DOCUMENTS;
use exo_env::MapEnvironment;

use super::{build::BuildError, update::report_update_needed};
use crate::config::Config;

#[async_trait]
pub trait CommandDefinition {
    fn command(&self) -> Command;

    async fn execute(&self, matches: &ArgMatches, _config: &Config) -> Result<()>;

    // Offer to opt-out of update notifications (for example, if the command is `exo update`)
    async fn is_update_report_needed(&self) -> bool {
        true
    }
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

    async fn execute(&self, matches: &ArgMatches, config: &Config) -> Result<()> {
        let subcommand = matches.subcommand().unwrap();

        let command_definition = self
            .command_definitions
            .iter()
            .find(|command_definition| command_definition.command().get_name() == subcommand.0);

        match command_definition {
            Some(command_definition) => {
                if command_definition.is_update_report_needed().await {
                    report_update_needed().await?;
                }
                command_definition.execute(subcommand.1, config).await
            }
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

pub(crate) fn default_trusted_documents_dir() -> PathBuf {
    PathBuf::from("trusted-documents")
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

pub fn yes_arg() -> Arg {
    Arg::new("yes")
        .help("Assume yes to all prompts")
        .long("yes")
        .short('y')
        .required(false)
        .num_args(0)
}

pub fn yes_value(matches: &ArgMatches) -> bool {
    get(matches, "yes").unwrap_or(false)
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
        .help("The PostgreSQL database connection string to use. If not specified, the program will attempt to read it from the environment (`EXO_POSTGRES_URL` or `DATABASE_URL`).")
        .long("database")
        .required(false)
}

pub fn database_value(matches: &ArgMatches) -> Option<String> {
    get(matches, "database")
}

pub fn output_arg() -> Arg {
    Arg::new("output")
        .help("Output file path")
        .long_help("If specified, the output will be written to this file path instead of stdout.")
        .short('o')
        .long("output")
        .required(false)
        .value_parser(clap::value_parser!(PathBuf))
        .num_args(1)
}

pub fn migration_scope_arg() -> Arg {
    Arg::new("scope")
        .help("The migration/import scope")
        .long("scope")
        .required(false)
        .value_parser(clap::value_parser!(String))
        .num_args(1)
}

pub fn migration_scope_value(matches: &ArgMatches) -> Option<String> {
    get(matches, "scope")
}

pub fn read_write_arg() -> Arg {
    Arg::new("read-write")
        .help("Run in read-write mode")
        .long("read-write")
        .action(clap::ArgAction::SetTrue)
        .required(false)
        .num_args(0)
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

pub(crate) fn enforce_trusted_documents_arg() -> Arg {
    Arg::new("enforce-trusted-documents")
        .help("Enforce trusted documents")
        .long_help("If set, the server will enforce that all documents are trusted.")
        .long("enforce-trusted-documents")
        .default_value("true")
        .required(false)
}

pub(crate) fn seed_arg() -> Arg {
    Arg::new("seed")
        .help("Seed the database")
        .long_help("If set, the database will be seeded with test data in the SQL format.")
        .long("seed")
        .required(false)
        .value_parser(clap::value_parser!(PathBuf))
        .num_args(1)
}

pub(crate) fn query_access_arg() -> Arg {
    Arg::new("query-access")
        .help("Access expression to apply to all queries")
        .long("query-access")
        .required(false)
        .value_parser(clap::value_parser!(bool))
        .num_args(1)
}

pub(crate) fn query_access_value(matches: &ArgMatches) -> bool {
    get(matches, "query-access").unwrap_or(false)
}

pub(crate) fn mutation_access_arg() -> Arg {
    Arg::new("mutation-access")
        .help("Access expression to apply to all mutations")
        .long("mutation-access")
        .required(false)
        .value_parser(clap::value_parser!(bool))
        .num_args(1)
}

pub(crate) fn mutation_access_value(matches: &ArgMatches) -> bool {
    get(matches, "mutation-access").unwrap_or(false)
}

pub(crate) fn setup_trusted_documents_enforcement(
    matches: &ArgMatches,
    env_vars: &mut MapEnvironment,
) {
    let enforce_trusted_documents: bool = get::<String>(matches, "enforce-trusted-documents")
        .map(|value| value != "false")
        .unwrap_or(false);

    if !enforce_trusted_documents {
        println!(
            "{}",
            "Trusting all documents due to --enforce-trusted-documents=false"
                .red()
                .bold()
        );
        env_vars.set(_EXO_ENFORCE_TRUSTED_DOCUMENTS, "false");
    }
}
