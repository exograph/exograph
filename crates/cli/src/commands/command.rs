use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::{Arg, ArgMatches, Command};

pub trait CommandDefinition {
    fn command(&self) -> Command;

    fn execute(&self, matches: &ArgMatches) -> Result<()>;
}
pub struct SubcommandDefinition {
    pub name: &'static str,
    pub about: &'static str,
    pub command_definitions: Vec<Box<dyn CommandDefinition>>,
}

impl SubcommandDefinition {
    pub fn new(
        name: &'static str,
        about: &'static str,
        command_definitions: Vec<Box<dyn CommandDefinition>>,
    ) -> Self {
        Self {
            name,
            about,
            command_definitions,
        }
    }
}

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

    fn execute(&self, matches: &ArgMatches) -> Result<()> {
        let subcommand = matches.subcommand().unwrap();
        for command_definition in &self.command_definitions {
            if command_definition.command().get_name() == subcommand.0 {
                return command_definition.execute(subcommand.1);
            }
        }

        Err(anyhow!("Unknown subcommand: {}", subcommand.0))
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

const DEFAULT_MODEL_FILE: &str = "index.exo";

pub fn model_file_arg() -> Arg {
    Arg::new("model")
        .help("The path to the Exograph model file.")
        .hide_default_value(false)
        .required(false)
        .value_parser(clap::value_parser!(PathBuf))
        .default_value(DEFAULT_MODEL_FILE)
        .index(1)
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
