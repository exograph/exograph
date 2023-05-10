// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::Result;
use async_recursion::async_recursion;
use clap::{ArgMatches, Command};
use colored::Colorize;
use std::{path::PathBuf, sync::atomic::Ordering};

use crate::util::watcher;

use super::{
    command::{default_model_file, ensure_exo_project_dir, get, port_arg, CommandDefinition},
    schema::migration_helper::migration_statements,
};
use exo_sql::{
    schema::spec::SchemaSpec,
    testing::db::{EphemeralDatabase, EphemeralDatabaseLauncher},
    Database,
};
use futures::FutureExt;

pub struct YoloCommandDefinition {}

impl CommandDefinition for YoloCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("yolo")
            .about("Run local exograph server with a temporary database")
            .arg(port_arg())
    }

    /// Run local exograph server with a temporary database
    fn execute(&self, matches: &ArgMatches) -> Result<()> {
        ensure_exo_project_dir(&PathBuf::from("."))?;

        let model: PathBuf = default_model_file();
        let port: Option<u32> = get(matches, "port");

        run(&model, port)
    }
}

fn run(model: &PathBuf, port: Option<u32>) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .unwrap();

    // make sure we do not exit on SIGINT
    // we spawn processes/containers that need to be cleaned up through drop(),
    // which does not run on a normal SIGINT exit
    crate::EXIT_ON_SIGINT.store(false, Ordering::SeqCst);

    let db_server = EphemeralDatabaseLauncher::create_server()?;
    let db = db_server.create_database("yolo")?;

    let jwt_secret = super::util::generate_random_string();

    let prestart_callback = || run_server(model, &jwt_secret, db.as_ref()).boxed();

    rt.block_on(watcher::start_watcher(model, port, prestart_callback))
}

#[async_recursion]
async fn run_server(
    model: &PathBuf,
    jwt_secret: &str,
    db: &(dyn EphemeralDatabase + Send + Sync),
) -> Result<()> {
    // set envs for server
    std::env::set_var("EXO_POSTGRES_URL", &db.url());
    std::env::remove_var("EXO_POSTGRES_USER");
    std::env::remove_var("EXO_POSTGRES_PASSWORD");
    std::env::set_var("EXO_INTROSPECTION", "true");
    std::env::set_var("EXO_JWT_SECRET", jwt_secret);
    std::env::set_var("EXO_CORS_DOMAINS", "*");

    println!("JWT secret is {}", &jwt_secret.cyan());
    println!("Postgres URL is {}", &db.url().cyan());

    // generate migrations for current database
    println!("Generating migrations...");
    let database = Database::from_env(None)?;

    let old_schema = {
        let client = database.get_client().await?;
        SchemaSpec::from_db(&client).await
    }?;

    for issue in &old_schema.issues {
        println!("{issue}");
    }

    let new_postgres_subsystem = super::schema::util::create_postgres_system(model)?;
    let new_schema = SchemaSpec::from_model(new_postgres_subsystem.tables.into_iter().collect());

    let migrations = migration_statements(&old_schema.value, &new_schema);

    // execute migration
    let result: Result<()> = {
        println!("Running migrations...");
        migrations.apply(&database, true).await
    };

    const CONTINUE: &str = "Continue with old schema";
    const REBUILD: &str = "Rebuild Postgres schema (wipe out all data)";
    const PAUSE: &str = "Pause for manual repair";
    const EXIT: &str = "Exit";

    if let Err(e) = result {
        println!("Error while applying migration: {e}");
        let options = vec![CONTINUE, REBUILD, PAUSE, EXIT];
        let ans = inquire::Select::new("Choose an option:", options).prompt()?;

        match ans {
            CONTINUE => {
                println!("Continuing with old incompatible schema...");
            }
            REBUILD => {
                let client = database.get_client().await?;
                client
                    .execute("DROP SCHEMA public CASCADE", &[])
                    .await
                    .expect("Failed to drop schema");
                client
                    .execute("CREATE SCHEMA public", &[])
                    .await
                    .expect("Failed to create schema");

                run_server(model, jwt_secret, db).await?;
            }
            PAUSE => {
                println!("Pausing for manual repair");
                println!(
                    "You can get the migrations by running: {}{}{}",
                    "EXO_POSTGRES_URL=".cyan(),
                    db.url().cyan(),
                    " exo schema migration".cyan()
                );
                println!("Press enter to continue.");
                let mut input: String = String::new();
                std::io::stdin().read_line(&mut input)?;
            }
            EXIT => {
                println!("Exiting...");
                let _ = crate::SIGINT.0.send(());
            }
            _ => unreachable!(),
        }
    }

    Ok(())
}
