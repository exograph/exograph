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
use async_trait::async_trait;
use clap::{ArgMatches, Command};
use colored::Colorize;
use common::env_const::{
    EXO_CORS_DOMAINS, EXO_INTROSPECTION, EXO_INTROSPECTION_LIVE_UPDATE, _EXO_DEPLOYMENT_MODE,
};
use postgres_model::migration::{self, Migration};
use std::{
    path::{Path, PathBuf},
    sync::atomic::Ordering,
};

use crate::{
    commands::{
        schema::{migrate::open_database, util},
        util::wait_for_enter,
    },
    util::watcher,
};
use common::env_const::{EXO_JWT_SECRET, EXO_POSTGRES_URL};

use super::command::{
    default_model_file, enforce_trusted_documents_arg, ensure_exo_project_dir, get, port_arg,
    setup_trusted_documents_enforcement, CommandDefinition,
};
use common::env_const::EXO_OIDC_URL;
use exo_sql::testing::db::{EphemeralDatabase, EphemeralDatabaseLauncher};
use futures::FutureExt;

enum JWTSecret {
    EnvSecret(String),
    EnvOidc(String),
    Generated(String),
}

pub struct YoloCommandDefinition {}

#[async_trait]
impl CommandDefinition for YoloCommandDefinition {
    fn command(&self) -> Command {
        Command::new("yolo")
            .about("Run local exograph server with a temporary database")
            .arg(port_arg())
            .arg(enforce_trusted_documents_arg())
    }

    /// Run local exograph server with a temporary database
    async fn execute(&self, matches: &ArgMatches) -> Result<()> {
        let root_path = PathBuf::from(".");
        ensure_exo_project_dir(&root_path)?;

        let port: Option<u32> = get(matches, "port");

        setup_trusted_documents_enforcement(matches);

        run(&root_path, port).await
    }
}

async fn run(root_path: &Path, port: Option<u32>) -> Result<()> {
    // make sure we do not exit on SIGINT
    // we spawn processes/containers that need to be cleaned up through drop(),
    // which does not run on a normal SIGINT exit
    crate::EXIT_ON_SIGINT.store(false, Ordering::SeqCst);

    let model: PathBuf = default_model_file();

    let db_server = EphemeralDatabaseLauncher::create_server()?;
    let db = db_server.create_database("yolo")?;

    let jwt_secret = std::env::var(EXO_JWT_SECRET).ok();
    let oidc_url = std::env::var(EXO_OIDC_URL).ok();

    let jwt_secret = match (jwt_secret, oidc_url) {
        (Some(_), Some(_)) => Err(anyhow::anyhow!(
            "Both {} and {} are set. Please unset one of them.",
            EXO_JWT_SECRET,
            EXO_OIDC_URL
        )),
        (Some(s), None) => Ok(JWTSecret::EnvSecret(s)),
        (None, Some(s)) => Ok(JWTSecret::EnvOidc(s)),
        (None, None) => Ok(JWTSecret::Generated(super::util::generate_random_string())),
    }?;

    let prestart_callback = || run_server(&model, &jwt_secret, db.as_ref()).boxed();

    watcher::start_watcher(root_path, port, prestart_callback).await
}

#[async_recursion]
async fn run_server(
    model: &PathBuf,
    jwt_secret: &JWTSecret,
    db: &(dyn EphemeralDatabase + Send + Sync),
) -> Result<()> {
    // set envs for server
    std::env::set_var(EXO_POSTGRES_URL, db.url());
    std::env::set_var(EXO_INTROSPECTION, "true");
    std::env::set_var(EXO_INTROSPECTION_LIVE_UPDATE, "true");
    std::env::set_var(_EXO_DEPLOYMENT_MODE, "yolo");

    match jwt_secret {
        JWTSecret::EnvSecret(s) => std::env::set_var(EXO_JWT_SECRET, s),
        JWTSecret::Generated(s) => std::env::set_var(EXO_JWT_SECRET, s),
        JWTSecret::EnvOidc(s) => std::env::set_var(EXO_OIDC_URL, s),
    };
    std::env::set_var(EXO_CORS_DOMAINS, "*");

    println!(
        "{}",
        "Starting with a temporary database (will be wiped out when the server exits)...".purple()
    );

    println!("Postgres URL: {}", &db.url().cyan());

    match jwt_secret {
        JWTSecret::EnvSecret(_) => {
            println!(
                "JWT secret: {}",
                "Using the EXO_JWT_SECRET env value".cyan()
            )
        }
        JWTSecret::EnvOidc(_) => {
            println!("OIDC URL: {}", "Using the EXO_OIDC_URL env value".cyan())
        }
        JWTSecret::Generated(s) => {
            println!("Generated JWT secret: {}", s.cyan())
        }
    };

    let db_client = open_database(None).await?;

    // generate migrations for current database
    let postgres_subsystem = util::create_postgres_system(model, None, false).await?;
    let migrations = Migration::from_db_and_model(&db_client, &postgres_subsystem).await?;

    // execute migration
    println!("Applying migrations...");
    let result = migrations.apply(&db_client, true).await;

    const CONTINUE: &str = "Continue with old schema";
    const REBUILD: &str = "Rebuild Postgres schema (wipe out all data)";
    const PAUSE: &str = "Pause for manual repair";
    const EXIT: &str = "Exit";

    if let Err(e) = result {
        println!("Error while applying migration: {e}");
        let options = vec![CONTINUE, REBUILD, PAUSE, EXIT];
        let ans = inquire::Select::new("Choose an option:", options).prompt()?;
        let db_client = open_database(None).await?;

        match ans {
            CONTINUE => {
                println!("Continuing with old incompatible schema...");
            }
            REBUILD => {
                migration::wipe_database(&db_client).await?;
                run_server(model, jwt_secret, db).await?;
            }
            PAUSE => {
                println!("Pausing for manual repair");
                println!(
                    "You can get the migrations by running: {}{}{}",
                    format!("{EXO_POSTGRES_URL}=").cyan(),
                    db.url().cyan(),
                    " exo schema migration".cyan()
                );
                wait_for_enter("Press enter to continue.")?;
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
