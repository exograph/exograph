use anyhow::{anyhow, Result};
use clap::{ArgMatches, Command};
use std::{io::Write, path::PathBuf, sync::atomic::Ordering};

use crate::util::watcher;

use super::{
    command::{get, get_required, model_file_arg, port_arg, CommandDefinition},
    schema::migration_helper::migration_statements,
};
use exo_sql::{schema::spec::SchemaSpec, Database};
use futures::FutureExt;

pub struct YoloCommandDefinition {}

impl CommandDefinition for YoloCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("yolo")
            .about("Run local exograph server with a temporary database")
            .arg(model_file_arg())
            .arg(port_arg())
    }

    /// Run local exograph server with a temporary database
    fn execute(&self, matches: &ArgMatches) -> Result<()> {
        let model: PathBuf = get_required(matches, "model")?;
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
    // we spawn containers that need to be cleaned up through drop(),
    // which does not run on a normal SIGINT exit
    crate::EXIT_ON_SIGINT.store(false, Ordering::SeqCst);

    let db = super::db::create_db()?;

    let jwt_secret = super::util::generate_random_string();

    let prestart_callback = || {
        async {
            // set envs for server
            std::env::set_var("EXO_POSTGRES_URL", &db.url());
            std::env::remove_var("EXO_POSTGRES_USER");
            std::env::remove_var("EXO_POSTGRES_PASSWORD");
            std::env::set_var("EXO_INTROSPECTION", "true");
            std::env::set_var("EXO_JWT_SECRET", &jwt_secret);
            std::env::set_var("EXO_CORS_DOMAINS", "*");

            println!("JWT secret is {}", &jwt_secret);
            println!("Postgres URL is {}", &db.url());

            // generate migrations for current database
            println!("Generating migrations...");
            let database = Database::from_env(None)?;

            let old_schema =  {
                let client = database.get_client().await?;
                SchemaSpec::from_db(&client).await
            }?;

            for issue in &old_schema.issues {
                println!("{issue}");
            }

            let new_postgres_subsystem = super::schema::util::create_postgres_system(model)?;
            let new_schema =
                SchemaSpec::from_model(new_postgres_subsystem.tables.into_iter().collect());

            let statements = migration_statements(&old_schema.value, &new_schema);

            // execute migration
            let result: Result<()> = {
                println!("Running migrations...");
                let mut client = database.get_client().await?;
                let transaction = client.transaction().await?;
                for (statement, _) in statements {
                    transaction.execute(&statement, &[]).await?;
                }
                transaction.commit().await.map_err(|e| anyhow!(e))
            };

            if let Err(e) = result {
                println!("Error while applying migration: {e}");
                println!("Choose an option:");
                print!("[c]ontinue without applying, (r)ebuild docker, (p)ause for manual repair, or (e)xit: ");
                std::io::stdout().flush()?;

                let mut input: String = String::new();
                let result = std::io::stdin().read_line(&mut input).map(|_| input.trim());

                match result {
                    // rebuild docker
                    Ok("r") => {
                        run(model, port)?;
                    }

                    // pause for manual repair
                    Ok("p") => {
                        println!("=====");
                        println!(
                            "Pausing for manual repair. Postgres URL is {}",
                            db.url()
                        );
                        println!("Press enter to continue.");
                        println!("=====");
                        std::io::stdin().read_line(&mut input)?;
                    }

                    // exit
                    Ok("e") => {
                        println!("Exiting...");
                        let _ = crate::SIGINT.0.send(());
                    }

                    // continue, do nothing
                    _ => {
                        println!("Continuing...");
                    }
                }
            }

            Ok(())
        }.boxed()
    };

    rt.block_on(watcher::start_watcher(model, port, prestart_callback))
}
