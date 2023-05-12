// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use clap::{ArgMatches, Command};
use colored::Colorize;
use futures::FutureExt;
use std::path::PathBuf;

use super::command::{get, port_arg, CommandDefinition};
use crate::{
    commands::{
        command::{default_model_file, ensure_exo_project_dir},
        schema::{migration::Migration, verify::VerificationErrors},
        util::wait_for_enter,
    },
    util::watcher,
};

pub struct DevCommandDefinition {}

#[async_trait]
impl CommandDefinition for DevCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("dev")
            .about("Run exograph server in development mode")
            .arg(port_arg())
    }

    /// Run local exograph server
    async fn execute(&self, matches: &ArgMatches) -> Result<()> {
        ensure_exo_project_dir(&PathBuf::from("."))?;

        let model: PathBuf = default_model_file();
        let port: Option<u32> = get(matches, "port");

        println!(
            "{}",
            "Starting server in development mode...".purple().bold()
        );
        // In the serve mode, which is meant for development, always enable introspection and use relaxed CORS
        std::env::set_var("EXO_INTROSPECTION", "true");
        std::env::set_var("EXO_CORS_DOMAINS", "*");

        const MIGRATE: &str = "Attempt migration";
        const CONTINUE: &str = "Continue with old schema";
        const PAUSE: &str = "Pause for manual repair";
        const EXIT: &str = "Exit";

        watcher::start_watcher(&model, port, || async {
            println!("{}", "\nVerifying new model...".blue().bold());

            loop {
                let verification_result = Migration::verify(None, &model).await;

                match verification_result {
                    Err(e @ VerificationErrors::ModelNotCompatible(_)) => {
                        let migrations = Migration::from_db_and_model(None, &model).await?;

                        // If migrations are safe to apply, let's go ahead with those
                        if !migrations.has_destructive_changes() {
                            if apply_migration(&migrations).await? {
                                break Ok(());
                            } else {
                                // Migration failed, perhaps due to adding a non-nullable column and table already had rows
                                continue;
                            }
                        }

                        println!("{}", "The schema of the current database is not compatible with the current model for the following reasons:".red().bold());
                        println!("{}", e.to_string().red().bold());

                        let options = vec![MIGRATE, CONTINUE, PAUSE, EXIT];
                        let ans = inquire::Select::new("Choose an option:", options).prompt()?;

                        match ans {
                            MIGRATE => {
                                println!("{}", "Attempting migration...".blue().bold());

                                // We will reach here only if the migration has some destructive changes (we auto-apply safe migrations; see above)
                                let allow_destructive_changes =
                                    inquire::Confirm::new("This migration contains destructive changes. Do you still want to proceed?")
                                    .with_default(false)
                                    .prompt()?;

                                if !allow_destructive_changes {
                                    println!("{}", "Aborting migration...".red().bold());
                                    continue;
                                }

                                if apply_migration(&migrations).await? {
                                    break Ok(());
                                } else {
                                    continue;
                                }
                            }
                            CONTINUE => {
                                println!("{}", "Continuing...".green().bold());
                                break Ok(());
                            }
                            PAUSE => {
                                wait_for_enter(&"Paused. Press enter to re-verify.".blue().bold())?;
                            }
                            EXIT => {
                                println!("Exiting...");
                                let _ = crate::SIGINT.0.send(());
                                break Ok(());

                            }
                            _ => unreachable!(),
                        }
                    }
                    _ => {
                        break verification_result
                            .map_err(|e| anyhow!("Verification failed: {}", e))
                    }
                }
            }
        }.boxed()).await
    }
}

async fn apply_migration(migrations: &Migration) -> Result<bool> {
    println!("{}", "Applying migration...".blue().bold());
    let result = migrations.apply(None, true).await;
    match result {
        Ok(_) => {
            println!("{}", "Migration successful!".green().bold());
            Ok(true)
        }
        Err(e) => {
            println!("{}", "Migration failed!".red().bold());
            println!("{}", e.to_string().red().bold());
            wait_for_enter(&"Press enter to re-verify.".blue().bold())?;
            Ok(false)
        }
    }
}
