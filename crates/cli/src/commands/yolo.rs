use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::Stdio,
    sync::atomic::Ordering,
    time::SystemTime,
};

use crate::util::watcher;

use super::{command::Command, schema::migration_helper::migration_statements};
use anyhow::{anyhow, Context, Result};
use futures::FutureExt;
use payas_sql::{schema::spec::SchemaSpec, Database};
use rand::Rng;

fn generate_random_string() -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(15)
        .map(char::from)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Run local claytip server with a temporary database
pub struct YoloCommand {
    pub model: PathBuf,
    pub port: Option<u32>,
}

impl Command for YoloCommand {
    fn run(&self, _system_start_time: Option<SystemTime>) -> Result<()> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .unwrap();

        // make sure we do not exit on SIGINT
        // we spawn containers that need to be cleaned up through drop(),
        // which does not run on a normal SIGINT exit
        crate::EXIT_ON_SIGINT.store(false, Ordering::SeqCst);

        // create postgresql docker
        let db = PostgreSQLInstance::from_docker()
            .context("While trying to instantiate PostgreSQL docker")?;

        let jwt_secret = generate_random_string();

        let prestart_callback = || {
            async {
            // set envs for server
            std::env::set_var("CLAY_DATABASE_URL", &db.connection_url);
            std::env::remove_var("CLAY_DATABASE_USER");
            std::env::remove_var("CLAY_DATABASE_PASSWORD");
            std::env::set_var("CLAY_INTROSPECTION", "true");
            std::env::set_var("CLAY_JWT_SECRET", &jwt_secret);
            std::env::set_var("CLAY_CORS_DOMAINS", "*");

            println!("JWT secret is {}", &jwt_secret);
            println!("Postgres URL is {}", &db.connection_url);

            // generate migrations for current database
            println!("Generating migrations...");
            let database = Database::from_env(None)?;

            let old_schema =  {
                let client = database.get_client().await?;
                SchemaSpec::from_db(&client).await
            }?;

            for issue in &old_schema.issues {
                println!("{}", issue);
            }

            let new_postgres_subsystem = crate::schema::util::create_postgres_system(&self.model)?;
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
                println!("Error while applying migration: {}", e);
                println!("Choose an option:");
                print!("[c]ontinue without applying, (r)ebuild docker, (p)ause for manual repair, or (e)xit: ");
                std::io::stdout().flush()?;

                let mut input: String = String::new();
                let result = std::io::stdin().read_line(&mut input).map(|_| input.trim());

                match result {
                    // rebuild docker
                    Ok("r") => {
                        self.run(_system_start_time)?;
                    }

                    // pause for manual repair
                    Ok("p") => {
                        println!("=====");
                        println!(
                            "Pausing for manual repair. Postgres URL is {}",
                            db.connection_url
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

        rt.block_on(watcher::start_watcher(
            &self.model,
            self.port,
            prestart_callback,
        ))
    }
}

struct PostgreSQLInstance {
    container_name: String,
    connection_url: String,
}

impl PostgreSQLInstance {
    pub fn from_docker() -> Result<PostgreSQLInstance> {
        println!("Starting PostgreSQL docker...");

        // acquire an empty port
        let port = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
            let addr = listener.local_addr()?;
            addr.port()
        };

        // generate container name
        let container_name = format!("claytip-yolo-{}", generate_random_string());

        // start postgres docker in background
        let mut db_background = std::process::Command::new("docker");
        let db_background = db_background
            .args([
                "run",
                "--rm",
                "--name",
                &container_name,
                "-e",
                "POSTGRES_USER=clay",
                "-e",
                "POSTGRES_PASSWORD=clay",
                "-p",
                &format!("{}:5432", port),
                "postgres",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut db_background = db_background
            .spawn()
            .context("While trying to start Docker (it may not be installed)")?;

        // let things stabilize

        let stderr = db_background.stderr.take().unwrap();
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            let line = line?;
            if line.contains("database system is ready to accept connections") {
                break;
            }
        }

        Ok(PostgreSQLInstance {
            container_name,
            connection_url: format!("postgresql://clay:clay@127.0.0.1:{}/postgres", port),
        })
    }
}

impl Drop for PostgreSQLInstance {
    fn drop(&mut self) {
        println!("Cleaning up container...");

        // kill docker, will get removed automatically on exit due to --rm
        std::process::Command::new("docker")
            .args(["kill", &self.container_name])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
    }
}
