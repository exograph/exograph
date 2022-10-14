use std::{
    env,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::SystemTime,
};

use anyhow::{anyhow, Result};
use clap::{Arg, Command};
use commands::{serve::ServeCommand, test::TestCommand, yolo::YoloCommand};

use crate::commands::{build::BuildCommand, schema};

mod commands;
pub(crate) mod util;

const DEFAULT_MODEL_FILE: &str = "index.clay";

pub static SIGINT: AtomicBool = AtomicBool::new(false);
pub static EXIT_ON_SIGINT: AtomicBool = AtomicBool::new(true);

fn model_file_arg() -> Arg<'static> {
    Arg::new("model")
        .help("The path to the Claytip model file.")
        .hide_default_value(false)
        .required(false)
        .value_parser(clap::value_parser!(PathBuf))
        .default_value(DEFAULT_MODEL_FILE)
        .index(1)
}

fn database_arg() -> Arg<'static> {
    Arg::new("database")
        .help("The PostgreSQL database connection string to use. If not specified, the program will attempt to read it from the environment (`CLAY_DATABASE_URL`).")
        .long("database")
        .required(false)
}

fn output_arg() -> Arg<'static> {
    Arg::new("output")
        .help("Output file path")
        .help("If specified, the output will be written to this file path instead of stdout.")
        .short('o')
        .long("output")
        .required(false)
        .value_parser(clap::value_parser!(PathBuf))
        .takes_value(true)
}

fn port_arg() -> Arg<'static> {
    Arg::new("port")
        .help("Listen port")
        .long_help("The port the server should listen for HTTP requests on.")
        .short('p')
        .long("port")
        .required(false)
        .value_parser(clap::value_parser!(u32))
        .takes_value(true)
}

fn main() -> Result<()> {
    let system_start_time = SystemTime::now();

    // register a sigint handler
    ctrlc::set_handler(move || {
        // set SIGINT flag when receiving signal
        SIGINT.store(true, Ordering::SeqCst);

        // exit if EXIT_ON_SIGINT is set
        // code may set this to be false if they have resources to
        // clean up before exiting
        if EXIT_ON_SIGINT.load(Ordering::SeqCst) {
            std::process::exit(0);
        }
    })
    .expect("Error setting Ctrl-C handler");

    let matches = Command::new("Claytip")
        .version(env!("CARGO_PKG_VERSION"))
        .disable_help_subcommand(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("build")
                .about("Build claytip server binary")
                .arg(model_file_arg()),
        )
        .subcommand(
            Command::new("schema")
                .about("Create, migrate, import, and verify database schema")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("create")
                        .about("Create a database schema from a Claytip model")
                        .arg(model_file_arg())
                        .arg(output_arg())
                )
                .subcommand(
                    Command::new("verify")
                        .about("Verify that the database schema is compatible with a Claytip model")
                        .arg(model_file_arg())
                        .arg(database_arg())
                )
                .subcommand(
                    Command::new("migrate")
                        .about("Produces a SQL migration script for a Claytip model and the specified database")
                        .arg(model_file_arg())
                        .arg(database_arg())
                        .arg(output_arg())
                        .arg(
                            Arg::new("allow-destructive-changes")
                                .help("By default, destructive changes in the model file are commented out. If specified, this option will uncomment such changes.")
                                .long("allow-destructive-changes")
                                .required(false)
                                .takes_value(false),
                        )

                )
                .subcommand(
                    Command::new("import")
                        .about("Create claytip model file based on a database schema")
                        .arg(database_arg())
                        .arg(output_arg()),
                ),
        )
        .subcommand(
            Command::new("serve")
                .about("Run claytip server in development mode")
                .arg(model_file_arg())
                .arg(port_arg()),
        )
        .subcommand(
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
                        .long("run-introspection-tests")
                )
        )
        .subcommand(
            Command::new("yolo")
                .about("Run local claytip server with a temporary database")
                .arg(model_file_arg())
                .arg(port_arg()),
        )
        .get_matches();

    fn get<T: Clone + Send + Sync + 'static>(
        matches: &clap::ArgMatches,
        arg_id: &str,
    ) -> Option<T> {
        matches.get_one::<T>(arg_id).cloned()
    }

    fn get_required<T: Clone + Send + Sync + 'static>(
        matches: &clap::ArgMatches,
        arg_id: &str,
    ) -> Result<T> {
        get(matches, arg_id).ok_or_else(|| anyhow!("Required argument `{}` is not present", arg_id))
    }

    // Map subcommands with args
    let command: Box<dyn crate::commands::command::Command> = match matches.subcommand() {
        Some(("build", matches)) => Box::new(BuildCommand {
            model: get_required(matches, "model")?,
        }),
        Some(("schema", matches)) => match matches.subcommand() {
            Some(("create", matches)) => Box::new(schema::create::CreateCommand {
                model: get_required(matches, "model")?,
                output: get(matches, "output"),
            }),
            Some(("verify", matches)) => Box::new(schema::verify::VerifyCommand {
                model: get_required(matches, "model")?,
                database: get(matches, "database"),
            }),
            Some(("import", matches)) => Box::new(schema::import::ImportCommand {
                output: get(matches, "output"),
            }),
            Some(("migrate", matches)) => Box::new(schema::migrate::MigrateCommand {
                model: get_required(matches, "model")?,
                database: get(matches, "database"),
                output: get(matches, "output"),
                comment_destructive_changes: !matches.contains_id("allow-destructive-changes"),
            }),
            _ => panic!("Unhandled command name"),
        },
        Some(("serve", matches)) => Box::new(ServeCommand {
            model: get_required(matches, "model")?,
            port: get(matches, "port"),
        }),
        Some(("test", matches)) => Box::new(TestCommand {
            dir: get_required(matches, "dir")?,
            pattern: get(matches, "pattern"),
            run_introspection_tests: matches.contains_id("run-introspection-tests"),
        }),
        Some(("yolo", matches)) => Box::new(YoloCommand {
            model: get_required(matches, "model")?,
            port: get(matches, "port"),
        }),
        _ => panic!("Unhandled command name"),
    };

    command.run(Some(system_start_time))
}
