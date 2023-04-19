use anyhow::{anyhow, Result};
use clap::{ArgMatches, Command};
use colored::Colorize;
use futures::FutureExt;
use std::{
    io::{stdin, stdout, Write},
    path::PathBuf,
};
use tokio::runtime::Runtime;

use super::command::{get, get_required, model_file_arg, port_arg, CommandDefinition};
use crate::{
    commands::schema::verify::{verify, VerificationErrors},
    util::watcher,
};

pub struct DevCommandDefinition {}

impl CommandDefinition for DevCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("dev")
            .about("Run exograph server in development mode")
            .arg(model_file_arg())
            .arg(port_arg())
    }

    /// Run local exograph server
    fn execute(&self, matches: &ArgMatches) -> Result<()> {
        let model: PathBuf = get_required(matches, "model")?;
        let port: Option<u32> = get(matches, "port");

        println!(
            "{}",
            "Starting server in development mode...".purple().bold()
        );
        // In the serve mode, which is meant for development, always enable introspection and use relaxed CORS
        std::env::set_var("EXO_INTROSPECTION", "true");
        std::env::set_var("EXO_CORS_DOMAINS", "*");

        let rt = Runtime::new()?;

        rt.block_on(watcher::start_watcher(&model, port, || async {
            println!("{}", "\nVerifying new model...".blue().bold());

            loop {
                let verification_result = verify(&model, None).await;

                match verification_result {
                    Err(e @ VerificationErrors::ModelNotCompatible(_)) => {
                        println!("{}", "The schema of the current database is not compatible with the current model for the following reasons:".red().bold());
                        println!("{}", e.to_string().red().bold());
                        println!("{}", "Select an option:".blue().bold());
                        print!("{}", "[c]ontinue without fixing, (p)ause and fix manually: ".blue().bold());
                        stdout().flush()?;

                        let mut input: String = String::new();
                        let result = std::io::stdin()
                            .read_line(&mut input)
                            .map(|_| input.trim())?;

                        match result {
                            "p" => {
                                println!("{}", "Paused. Press enter to re-verify.".blue().bold());

                                let mut line = String::new();
                                stdin().read_line(&mut line)?;
                            }
                            _ => {
                                println!("{}", "Continuing...".green().bold());
                                break Ok(());
                            }
                        }
                    }
                    _ => {
                        break verification_result
                            .map_err(|e| anyhow!("Verification failed: {}", e))
                    }
                }
            }
        }.boxed()))
    }
}
