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
                .arg(
                    Arg::new("model")
                        .help("Claytip model file")
                        .value_parser(clap::value_parser!(PathBuf))
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
                                .value_parser(clap::value_parser!(PathBuf))
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
                                .value_parser(clap::value_parser!(PathBuf))
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
                                .value_parser(clap::value_parser!(PathBuf))
                                .default_value(DEFAULT_MODEL_FILE)
                                .index(1),
                        )
                        .arg(
                            Arg::new("output")
                                .help("If specified, the script will be written to this filename")
                                .short('o')
                                .long("output")
                                .required(false)
                                .value_parser(clap::value_parser!(PathBuf))
                                .takes_value(true)
                        )
                )
                .subcommand(
                    Command::new("import")
                        .about("Create claytip model file based on a database schema")
                        .arg(
                            Arg::new("output")
                                .help("Claytip model output file")
                                .short('o')
                                .long("output")
                                .required(false)
                                .takes_value(true)
                                .value_parser(clap::value_parser!(PathBuf))
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
                        .value_parser(clap::value_parser!(PathBuf))
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
                        .value_parser(clap::value_parser!(PathBuf))
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
                        .value_parser(clap::value_parser!(PathBuf))
                        .index(1),

                ).arg(
                    Arg::new("port")
                        .help("Port to start the server")
                        .short('p')
                        .long("port")
                        .value_parser(clap::value_parser!(u32))
                        .takes_value(true)
                        .value_name("port"),
                ),
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
            }),
            Some(("verify", matches)) => Box::new(schema::verify::VerifyCommand {
                model: get_required(matches, "model")?,
                database: get_required(matches, "database")?,
            }),
            Some(("import", matches)) => Box::new(schema::import::ImportCommand {
                output: get_required(matches, "output")?,
            }),
            Some(("migrate", matches)) => Box::new(schema::migrate::MigrateCommand {
                model: get_required(matches, "model")?,
                output: get(matches, "output"),
                comment_destructive_changes: !matches.contains_id("allow-destructive-changes"),
            }),
            _ => panic!("Unhandled command name"),
        },
        Some(("serve", matches)) => Box::new(ServeCommand {
            model: get_required(matches, "model")?,
            port: matches.get_one::<u32>("port").copied(),
        }),
        Some(("test", matches)) => Box::new(TestCommand {
            dir: get_required(matches, "dir")?,
            pattern: get(matches, "pattern"),
        }),
        Some(("yolo", matches)) => Box::new(YoloCommand {
            model: get_required(matches, "model")?,
            port: get(matches, "port"),
        }),
        _ => panic!("Unhandled command name"),
    };

    command.run(Some(system_start_time))
}
