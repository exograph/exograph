use std::{env, path::PathBuf, time::SystemTime};

use anyhow::Result;
use clap::{Arg, Command};
use commands::{
    migrate::MigrateCommand, serve::ServeCommand, test::TestCommand, yolo::YoloCommand,
};

use crate::commands::{build::BuildCommand, import, schema};

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
            Command::new("migrate")
                .about("Perform a database migration for a claytip model")
                .arg(
                    Arg::new("model")
                        .help("Claytip model file")
                        .required(true)
                        .index(1),
                )
                .arg(
                    Arg::new("database")
                        .help("Database source (postgres, git)")
                        .required(true)
                        .index(2),
                ),
        )
        .subcommand(
            Command::new("model")
                .about("Claytip model utilities")
                .subcommand_required(true)
                .arg_required_else_help(true)
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
            Command::new("schema")
                .about("Database schema utilities")
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
                ),
        )
        .subcommand(
            Command::new("serve")
                .about("Run local claytip server")
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

    // Map subcommands with args
    let command: Box<dyn crate::commands::command::Command> = match matches.subcommand() {
        Some(("build", matches)) => Box::new(BuildCommand {
            model: PathBuf::from(matches.value_of("model").unwrap()),
        }),
        Some(("migrate", matches)) => Box::new(MigrateCommand {
            model: PathBuf::from(matches.value_of("model").unwrap()),
            database: matches.value_of("database").unwrap().to_owned(),
        }),
        Some(("model", matches)) => match matches.subcommand() {
            Some(("import", matches)) => Box::new(import::ImportCommand {
                output: PathBuf::from(matches.value_of("output").unwrap()),
            }),
            _ => panic!("Unhandled command name"),
        },
        Some(("schema", matches)) => match matches.subcommand() {
            Some(("create", matches)) => Box::new(schema::CreateCommand {
                model: PathBuf::from(matches.value_of("model").unwrap()),
            }),
            Some(("verify", matches)) => Box::new(schema::VerifyCommand {
                model: PathBuf::from(matches.value_of("model").unwrap()),
                database: matches.value_of("database").unwrap().to_owned(),
            }),
            _ => panic!("Unhandled command name"),
        },

        Some(("serve", matches)) => Box::new(ServeCommand {
            model: PathBuf::from(matches.value_of("model").unwrap()),
            port: {
                let port_str = matches.value_of("port");
                port_str.map(|port_str| match port_str.parse() {
                    Ok(port) => port,
                    Err(_) => {
                        eprintln!("Invalid port number '{port_str}'");
                        std::process::exit(1);
                    }
                })
            },
        }),
        Some(("test", matches)) => Box::new(TestCommand {
            dir: PathBuf::from(matches.value_of("dir").unwrap()),
            pattern: matches.value_of("pattern").map(|s| s.to_owned()),
        }),
        Some(("yolo", matches)) => Box::new(YoloCommand {
            model: PathBuf::from(matches.value_of("model").unwrap()),
        }),
        _ => panic!("Unhandled command name"),
    };

    command.run(Some(system_start_time))
}
