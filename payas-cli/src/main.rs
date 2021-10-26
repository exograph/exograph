use std::{env, path::PathBuf, time::SystemTime};

use anyhow::Result;
use clap::{App, AppSettings, Arg, SubCommand};

use crate::commands::{
    build::BuildCommand, import, schema, Command, MigrateCommand, ServeCommand, TestCommand,
    YoloCommand,
};

mod commands;

const DEFAULT_MODEL_FILE: &str = "index.clay";

fn main() -> Result<()> {
    let system_start_time = SystemTime::now();

    let matches = App::new("Claytip")
        .version(env!("CARGO_PKG_VERSION"))
        .global_setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            SubCommand::with_name("build")
                .about("Build claytip server binary")
                .arg(
                    Arg::with_name("model")
                        .help("Claytip model file")
                        .default_value(DEFAULT_MODEL_FILE)
                        .index(1),
                )
                .arg(
                    Arg::with_name("watch")
                        .help("Automatically build when model file changes")
                        .long("watch"),
                ),
        )
        .subcommand(
            SubCommand::with_name("migrate")
                .about("Perform a database migration for a claytip model")
                .arg(
                    Arg::with_name("model")
                        .help("Claytip model file")
                        .required(true)
                        .index(1),
                )
                .arg(
                    Arg::with_name("database")
                        .help("Database source (postgres, git)")
                        .required(true)
                        .index(2),
                ),
        )
        .subcommand(
            SubCommand::with_name("model")
                .about("Claytip model utilities")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommand(
                    SubCommand::with_name("import")
                        .about("Create claytip model file based on a database schema")
                        .arg(
                            Arg::with_name("output")
                                .help("Claytip model output file")
                                .short("o")
                                .long("output")
                                .takes_value(true)
                                .value_name("output")
                                .default_value(DEFAULT_MODEL_FILE),
                        ),
                ),
        )
        .subcommand(
            SubCommand::with_name("schema")
                .about("Database schema utilities")
                .setting(AppSettings::SubcommandRequiredElseHelp)
                .subcommand(
                    SubCommand::with_name("create")
                        .about("Create a database schema from a claytip model")
                        .arg(
                            Arg::with_name("model")
                                .help("Claytip model file")
                                .default_value(DEFAULT_MODEL_FILE)
                                .index(1),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("verify")
                        .about("Verify that a schema is compatible with a claytip model")
                        .arg(
                            Arg::with_name("model")
                                .help("Claytip model file")
                                .required(true)
                                .index(1),
                        )
                        .arg(
                            Arg::with_name("database")
                                .help("Database schema source (postgres, git)")
                                .required(true)
                                .index(2),
                        ),
                ),
        )
        .subcommand(
            SubCommand::with_name("serve")
                .about("Run local claytip server")
                .arg(
                    Arg::with_name("model")
                        .help("Claytip model file")
                        .default_value(DEFAULT_MODEL_FILE)
                        .index(1),
                )
                .arg(
                    Arg::with_name("watch")
                        .help("Automatically restart server when model file changes")
                        .long("watch"),
                ),
        )
        .subcommand(
            SubCommand::with_name("test")
                .about("Perform integration tests")
                .arg(
                    Arg::with_name("dir")
                        .help("Integration test directory")
                        .required(true)
                        .index(1),
                ),
        )
        .subcommand(
            SubCommand::with_name("yolo")
                .about("Run local claytip server with a temporary database")
                .arg(
                    Arg::with_name("model")
                        .help("Claytip model file")
                        .default_value(DEFAULT_MODEL_FILE)
                        .index(1),
                ),
        )
        .get_matches();

    // Map subcommands with args
    let command: Box<dyn Command> = match matches.subcommand() {
        ("build", Some(matches)) => Box::new(BuildCommand {
            model: PathBuf::from(matches.value_of("model").unwrap()),
            watch: matches.is_present("watch"),
        }),
        ("migrate", Some(matches)) => Box::new(MigrateCommand {
            model: PathBuf::from(matches.value_of("model").unwrap()),
            database: matches.value_of("database").unwrap().to_owned(),
        }),
        ("model", Some(matches)) => match matches.subcommand() {
            ("import", Some(matches)) => Box::new(import::ImportCommand {
                output: PathBuf::from(matches.value_of("output").unwrap()),
            }),
            _ => panic!("Unhandled command name"),
        },
        ("schema", Some(matches)) => match matches.subcommand() {
            ("create", Some(matches)) => Box::new(schema::CreateCommand {
                model: PathBuf::from(matches.value_of("model").unwrap()),
            }),
            ("verify", Some(matches)) => Box::new(schema::VerifyCommand {
                model: PathBuf::from(matches.value_of("model").unwrap()),
                database: matches.value_of("database").unwrap().to_owned(),
            }),
            _ => panic!("Unhandled command name"),
        },

        ("serve", Some(matches)) => Box::new(ServeCommand {
            model: PathBuf::from(matches.value_of("model").unwrap()),
            watch: matches.is_present("watch"),
        }),
        ("test", Some(matches)) => Box::new(TestCommand {
            dir: PathBuf::from(matches.value_of("dir").unwrap()),
        }),
        ("yolo", Some(matches)) => Box::new(YoloCommand {
            model: PathBuf::from(matches.value_of("model").unwrap()),
        }),
        _ => panic!("Unhandled command name"),
    };

    command.run(Some(system_start_time))
}
