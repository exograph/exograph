// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use clap::{Arg, ArgMatches, Command};
use colored::Colorize;
use common::env_const::{
    EXO_CORS_DOMAINS, EXO_ENV, EXO_INTROSPECTION, EXO_INTROSPECTION_LIVE_UPDATE,
};
use exo_env::{MapEnvironment, SystemEnvironment};
use exo_sql::DatabaseClient;
use exo_sql::schema::migration::{Migration, VerificationErrors};
use futures::FutureExt;
use std::{path::PathBuf, sync::Arc};

use super::command::{
    CommandDefinition, enforce_trusted_documents_arg, get, migration_scope_arg, port_arg,
};
use crate::commands::command::migration_scope_value;
use crate::config::{Config, WatchStage};
use crate::{
    commands::{
        command::{
            default_model_file, ensure_exo_project_dir, setup_trusted_documents_enforcement,
        },
        schema::util,
        util::wait_for_enter,
    },
    util::watcher,
};

use crate::commands::util::compute_migration_scope;

pub struct DevCommandDefinition {}

#[async_trait]
impl CommandDefinition for DevCommandDefinition {
    fn command(&self) -> Command {
        Command::new("dev")
            .about("Run exograph server in development mode")
            .arg(port_arg())
            .arg(enforce_trusted_documents_arg())
            .arg(migration_scope_arg())
            .arg(
                Arg::new("ignore-migration-errors")
                    .help("Ignore migration errors")
                    .long("ignore-migration-errors")
                    .required(false)
                    .num_args(0),
            )
    }

    /// Run local exograph server
    async fn execute(&self, matches: &ArgMatches, config: &Config) -> Result<()> {
        let root_path = PathBuf::from(".");
        ensure_exo_project_dir(&root_path)?;

        let model_file: PathBuf = default_model_file();
        let port: Option<u32> = get(matches, "port");

        let ignore_migration_errors: bool =
            get(matches, "ignore-migration-errors").unwrap_or(false);

        println!(
            "{}",
            "Starting server in development mode...".purple().bold()
        );

        let migration_scope_str = migration_scope_value(matches);

        // Create environment variables for the child server process
        let mut env_vars = MapEnvironment::new_with_fallback(Arc::new(SystemEnvironment));
        setup_trusted_documents_enforcement(matches, &mut env_vars);
        env_vars.set(EXO_INTROSPECTION, "true");
        env_vars.set(EXO_INTROSPECTION_LIVE_UPDATE, "true");
        env_vars.set(EXO_ENV, "dev");
        env_vars.set(EXO_CORS_DOMAINS, "*");

        const MIGRATE: &str = "Attempt migration";
        const CONTINUE: &str = "Continue with old schema";
        const PAUSE: &str = "Pause for manual repair";
        const EXIT: &str = "Exit";

        watcher::start_watcher(&root_path, port, config, Some(&WatchStage::Dev), move || {
            let model_file = model_file.clone();
            let migration_scope_str = migration_scope_str.clone();
            let env_vars = env_vars.clone();
            async move {
                let migration_scope = compute_migration_scope(migration_scope_str);
                println!("{}", "\nVerifying new model...".blue().bold());

                let db_client = util::open_database(None).await?;

                loop {
                    // Pass true as use_ir to use the IR model, since we just built the model in the watcher
                    let database = util::extract_postgres_database(&model_file, None, true).await?;
                    let mut db_client = db_client.get_client().await?;
                    let verification_result = Migration::verify(&db_client, &database, &migration_scope).await;

                    match verification_result {
                        Err(e @ VerificationErrors::ModelNotCompatible(_)) => {
                            let migrations = Migration::from_db_and_model(&db_client, &database, &migration_scope).await?;

                            // If migrations are safe to apply, let's go ahead with those
                            if !migrations.has_destructive_changes() {
                                if apply_migration(&mut db_client, &migrations, ignore_migration_errors).await? {
                                    break Ok(env_vars);
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

                                    if apply_migration(&mut db_client, &migrations, ignore_migration_errors).await? {
                                        break Ok(env_vars);
                                    } else {
                                        continue;
                                    }
                                }
                                CONTINUE => {
                                    println!("{}", "Continuing...".green().bold());
                                    break Ok(env_vars);
                                }
                                PAUSE => {
                                    wait_for_enter(&"Paused. Press enter to re-verify.".blue().bold())?;
                                }
                                EXIT => {
                                    println!("Exiting...");
                                    let _ = crate::SIGINT.0.send(());
                                    break Ok(env_vars);

                                }
                                _ => unreachable!(),
                            }
                        }
                        _ => {
                            break verification_result
                                .map_err(|e| anyhow!("Verification failed: {}", e))
                                .map(|_| env_vars)
                        }
                    }
                }
            }.boxed()
        }).await
    }
}

async fn apply_migration(
    db_client: &mut DatabaseClient,
    migrations: &Migration,
    ignore_migration_errors: bool,
) -> Result<bool> {
    println!("{}", "Applying migration...".blue().bold());
    let result = migrations.apply(db_client, true).await;
    match result {
        Ok(_) => {
            println!("{}", "Migration successful!".green().bold());
            Ok(true)
        }
        Err(e) => {
            println!("{}", "Migration failed!".red().bold());
            println!("{}", e.to_string().red().bold());

            if ignore_migration_errors {
                println!(
                    "{}",
                    "Continuing (ignoring migration errors)..."
                        .red()
                        .italic()
                        .bold()
                );
                Ok(true)
            } else {
                wait_for_enter(&"Press enter to re-verify.".blue().bold())?;
                Ok(false)
            }
        }
    }
}
