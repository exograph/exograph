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
use common::env_const::{DATABASE_URL, EXO_POSTGRES_READ_WRITE};
use common::env_processing::EnvProcessing;
use exo_env::{Environment, MapEnvironment};
use exo_sql::{TransactionMode, schema::migration::Migration};
use std::{
    path::PathBuf,
    sync::{Arc, atomic::Ordering},
};

use crate::{
    commands::{schema::util, util::wait_for_enter},
    config::{Config, WatchStage},
    util::watcher,
};
use common::env_const::{EXO_JWT_SECRET, EXO_POSTGRES_URL};

use super::command::{
    CommandDefinition, default_model_file, enforce_trusted_documents_arg, ensure_exo_project_dir,
    get, port_arg, seed_arg, setup_trusted_documents_enforcement,
};
use common::env_const::EXO_OIDC_URL;
use exo_sql::{
    schema::spec::MigrationScope,
    testing::db::{EphemeralDatabase, EphemeralDatabaseLauncher},
};
use futures::FutureExt;

#[derive(Clone)]
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
            .arg(seed_arg())
    }

    fn env_processing(&self, _env: &dyn Environment) -> EnvProcessing {
        EnvProcessing::Process(Some("yolo".to_string()))
    }

    /// Run local exograph server with a temporary database
    async fn execute(
        &self,
        matches: &ArgMatches,
        config: &Config,
        env: Arc<dyn Environment>,
    ) -> Result<()> {
        let mut env_vars = MapEnvironment::new_with_fallback(env);

        let root_path = PathBuf::from(".");
        ensure_exo_project_dir(&root_path)?;

        let port: Option<u32> = get(matches, "port");
        let seed_path: Option<PathBuf> = get(matches, "seed");

        // make sure we do not exit on SIGINT
        // we spawn processes/containers that need to be cleaned up through drop(),
        // which does not run on a normal SIGINT exit
        crate::EXIT_ON_SIGINT.store(false, Ordering::SeqCst);

        let model: PathBuf = default_model_file();

        let db_server = EphemeralDatabaseLauncher::from_env().create_server()?;
        let db = db_server.create_database("yolo")?;

        let jwt_secret = env_vars.get(EXO_JWT_SECRET);
        let oidc_url = env_vars.get(EXO_OIDC_URL);

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

        // Create environment variables for the child server process
        setup_trusted_documents_enforcement(matches, &mut env_vars);
        super::util::set_dev_yolo_env_vars(&mut env_vars, true);

        if env_vars.get(EXO_POSTGRES_URL).is_some() || env_vars.get(DATABASE_URL).is_some() {
            println!("{}", "Yolo mode ignores EXO_POSTGRES_URL and DATABASE_URL env vars. Creating a new ephemeral database instead.".yellow());
        }
        env_vars.set(EXO_POSTGRES_URL, &db.url());

        if env_vars.get(EXO_POSTGRES_READ_WRITE).is_some() {
            println!("{}", "Yolo mode ignores EXO_POSTGRES_READ_WRITE env. Using read-write mode with the ephemeral database it creates.".yellow());
        }
        env_vars.set(EXO_POSTGRES_READ_WRITE, "true");

        match &jwt_secret {
            JWTSecret::EnvSecret(s) => env_vars.set(EXO_JWT_SECRET, s),
            JWTSecret::Generated(s) => env_vars.set(EXO_JWT_SECRET, s),
            JWTSecret::EnvOidc(s) => env_vars.set(EXO_OIDC_URL, s),
        };

        let prestart_callback = || {
            setup_database(
                &model,
                &jwt_secret,
                db.as_ref(),
                seed_path.clone(),
                env_vars.clone(),
            )
            .boxed()
        };

        watcher::start_watcher(
            &root_path,
            port,
            config,
            Some(&WatchStage::Yolo),
            prestart_callback,
        )
        .await
    }
}

#[async_recursion]
async fn setup_database(
    model: &PathBuf,
    jwt_secret: &JWTSecret,
    db: &(dyn EphemeralDatabase + Send + Sync),
    seed: Option<PathBuf>,
    env_vars: MapEnvironment,
) -> Result<MapEnvironment> {
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

    let db_client =
        util::open_database(Some(&db.url()), TransactionMode::ReadWrite, &env_vars).await?;
    let mut db_client = db_client.get_client().await?;

    let database = util::extract_postgres_database(&model, None, false).await?;

    let migrations =
        Migration::from_db_and_model(&db_client, &database, &MigrationScope::FromNewSpec).await?;

    println!("{}", "Applying migration...".blue().bold());

    let migration_result = migrations.apply(&mut db_client, true).await;

    const CONTINUE: &str = "Continue with old schema";
    const REBUILD: &str = "Rebuild Postgres schema (wipe out all data)";
    const PAUSE: &str = "Pause for manual repair";
    const EXIT: &str = "Exit";

    if let Err(e) = migration_result {
        println!("Error while applying migration: {e}");
        let options = vec![CONTINUE, REBUILD, PAUSE, EXIT];
        let ans = inquire::Select::new("Choose an option:", options).prompt()?;
        let db_client =
            util::open_database(Some(&db.url()), TransactionMode::ReadWrite, &env_vars).await?;
        let mut db_client = db_client.get_client().await?;

        match ans {
            CONTINUE => {
                println!("Continuing with old incompatible schema...");
            }
            REBUILD => {
                exo_sql::schema::migration::wipe_database(&mut db_client).await?;
                setup_database(model, jwt_secret, db, None, env_vars.clone()).await?;
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
    } else if let Some(seed) = seed {
        let seed = std::fs::read_to_string(seed)?;
        db_client.batch_execute(&seed).await?;
    }

    Ok(env_vars)
}
