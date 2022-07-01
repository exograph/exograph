use std::{env, path::PathBuf, time::SystemTime};

use anyhow::Result;
use clap::{Arg, Command};
use commands::{serve::ServeCommand, test::TestCommand, yolo::YoloCommand};

use crate::commands::{build::BuildCommand, schema};

mod commands;

const DEFAULT_MODEL_FILE: &str = "index.clay";

fn main() -> Result<()> {
    let system_start_time = SystemTime::now();

    let matches = Command::new("Claytip")
        .version(env!("CARGO_PKG_VERSION"))
        .disable_help_subcommand(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("build")
                .about("Build claytip server binary")
                .arg(
                    Arg::new("model")
                        .help("Claytip model file")
                        .default_value(DEFAULT_MODEL_FILE)
                        .index(1),
                ),
        )
        .subcommand(
            Command::new("schema")
                .about("Create, migrate, import, and verify database schema")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("create")
                        .about("Create a database schema from a claytip model")
                        .arg(
                            Arg::new("model")
                                .help("Claytip model file")
                                .default_value(DEFAULT_MODEL_FILE)
                                .index(1),
                        ),
                )
                .subcommand(
                    Command::new("verify")
                        .about("Verify that a schema is compatible with a claytip model")
                        .arg(
                            Arg::new("model")
                                .help("Claytip model file")
                                .required(true)
                                .index(1),
                        )
                        .arg(
                            Arg::new("database")
                                .help("Database schema source (postgres, git)")
                                .required(true)
                                .index(2),
                        ),
                )
                .subcommand(
                    Command::new("migrate")
                        .about("Produces a SQL migration script for a claytip model and the provided database")
                        .arg(
                            Arg::new("allow-destructive-changes")
                                .help("Allow destructive changes (otherwise commented for manual review)")
                                .long("allow-destructive-changes")
                                .required(false)
                                .takes_value(false),
                        )
                        .arg(
                            Arg::new("model")
                                .help("Claytip model file")
                                .required(true)
                                .index(1),
                        ),
                )
                .subcommand(
                    Command::new("import")
                        .about("Create claytip model file based on a database schema")
                        .arg(
                            Arg::new("output")
                                .help("Claytip model output file")
                                .short('o')
                                .long("output")
                                .takes_value(true)
                                .value_name("output")
                                .default_value(DEFAULT_MODEL_FILE),
                        ),
                ),
        )
        .subcommand(
            Command::new("serve")
                .about("Run claytip server in development mode")
                .arg(
                    Arg::new("model")
                        .help("Claytip model file")
                        .default_value(DEFAULT_MODEL_FILE)
                        .index(1),
                )
                .arg(
                    Arg::new("port")
                        .help("Port to start the server")
                        .short('p')
                        .long("port")
                        .value_parser(clap::value_parser!(u32))
                        .takes_value(true)
                        .value_name("port"),
                ),
        )
        .subcommand(
            Command::new("test")
                .about("Perform integration tests")
                .arg(
                    Arg::new("dir")
                        .help("Integration test directory")
                        .required(true)
                        .index(1),
                )
                .arg(
                    Arg::new("pattern")
                        .help("glob pattern to choose tests to run")
                        .required(false)
                        .index(2),
                ),
        )
        .subcommand(
            Command::new("yolo")
                .about("Run local claytip server with a temporary database")
                .arg(
                    Arg::new("model")
                        .help("Claytip model file")
                        .default_value(DEFAULT_MODEL_FILE)
                        .index(1),
                ),
        )
        .get_matches();

    fn get_path(matches: &clap::ArgMatches, arg_id: &str) -> PathBuf {
        PathBuf::from(matches.get_one::<String>(arg_id).unwrap())
    }

    // Map subcommands with args
    let command: Box<dyn crate::commands::command::Command> = match matches.subcommand() {
        Some(("build", matches)) => Box::new(BuildCommand {
            model: get_path(matches, "model"),
        }),
        Some(("schema", matches)) => match matches.subcommand() {
            Some(("create", matches)) => Box::new(schema::create::CreateCommand {
                model: get_path(matches, "model"),
            }),
            Some(("verify", matches)) => Box::new(schema::verify::VerifyCommand {
                model: get_path(matches, "model"),
                database: matches.get_one::<String>("database").unwrap().to_owned(),
            }),
            Some(("import", matches)) => Box::new(schema::import::ImportCommand {
                output: get_path(matches, "output"),
            }),
            Some(("migrate", matches)) => Box::new(schema::migrate::MigrateCommand {
                model: get_path(matches, "model"),
                comment_destructive_changes: !matches.contains_id("allow-destructive-changes"),
            }),
            _ => panic!("Unhandled command name"),
        },
        Some(("serve", matches)) => Box::new(ServeCommand {
            model: get_path(matches, "model"),
            port: matches.get_one::<u32>("port").copied(),
        }),
        Some(("test", matches)) => Box::new(TestCommand {
            dir: get_path(matches, "dir"),
            pattern: matches.get_one::<String>("pattern").map(|s| s.to_owned()),
        }),
        Some(("yolo", matches)) => Box::new(YoloCommand {
            model: get_path(matches, "model"),
        }),
        _ => panic!("Unhandled command name"),
    };

    command.run(Some(system_start_time))
}
