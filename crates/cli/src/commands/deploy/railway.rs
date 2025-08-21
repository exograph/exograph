// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use clap::{Arg, Command};
use colored::Colorize;
use exo_env::Environment;

use crate::commands::{command::CommandDefinition, deploy::util::write_template_file};
use crate::config::Config;

pub(super) struct RailwayCommandDefinition {}

#[async_trait]
impl CommandDefinition for RailwayCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("railway").about("Deploy to Railway.app").arg(
            Arg::new("use-railway-db")
                .help("Use database provided by Railway.app")
                .long("use-railway-db")
                .value_parser(clap::value_parser!(bool)),
        )
    }

    /// Create a Dockerfile. Then provide instructions on how to deploy the app to Railway.app.
    async fn execute(
        &self,
        matches: &clap::ArgMatches,
        _config: &Config,
        _env: Arc<dyn Environment>,
    ) -> Result<()> {
        let current_dir = std::env::current_dir()?;

        let use_railway_db: bool = match matches.get_one("use-railway-db") {
            Some(value) => *value,
            None => {
                // The value was not provided as a command line argument. Prompt the user.
                const RAILWAY: &str = "Railway.app provided";
                const EXTERNAL: &str = "External";
                let options = vec![RAILWAY, EXTERNAL];
                let ans =
                    inquire::Select::new("Which database you want to use:", options).prompt()?;

                ans == RAILWAY
            }
        };

        create_dockerfile(&current_dir, use_railway_db)?;
        create_config_toml(&current_dir)?;

        println!(
            "{}",
            "\n- Push the repository to a (public or private) repository on GitHub".blue()
        );
        println!(
            "{}",
            "- Create a new project on Railway.app and connect it to the GitHub repository".blue()
        );

        if use_railway_db {
            println!(
                "{}",
                r#"- In the same project, choose "New" and then add a Postgres database"#.blue()
            );
            println!(
                "{}",
                "- Set the following environment variables in Railway.app:".blue()
            );
            println!(
                "\t{} to point to {}",
                "DATABASE_URL".green(),
                "${{Postgres.DATABASE_URL}}".yellow()
            );
            println!(
                "\t{} to point to {}",
                "DATABASE_PRIVATE_URL".green(),
                "${{Postgres.DATABASE_PRIVATE_URL}}".yellow()
            );
        } else {
            println!(
                "{}",
                "Set the following environment variables in Railway.app:".blue()
            );
            println!(
                "\t{} to point to {}",
                "DATABASE_URL".blue(),
                "<your external Postgres database URL>".yellow()
            );
        }

        println!(
            "{}",
            r#"- Go to the "Settings" -> "Networking" and click "Generate Domain"."#.blue()
        );
        println!("{}", "- Start Exograph in playground mode".blue());
        println!(
            "\t{}",
            "exo playground --endpoint <the generated domain>/graphql".yellow()
        );
        println!(
            "{}",
            r#"- Open the playground URL shown and execute GraphQL operations as usual"#.blue()
        );

        Ok(())
    }
}

static RAILWAY_TOML: &str = include_str!("../templates/railway.toml");
static DOCKERFILE_RAILWAY_DB: &str = include_str!("../templates/Dockerfile.railway.railway_db");
static DOCKERFILE_EXTERNAL_DB: &str = include_str!("../templates/Dockerfile.railway.external_db");

fn create_dockerfile(base_dir: &Path, use_railway_db: bool) -> Result<()> {
    let created = write_template_file(
        base_dir.join("Dockerfile.railway"),
        if use_railway_db {
            DOCKERFILE_RAILWAY_DB
        } else {
            DOCKERFILE_EXTERNAL_DB
        },
        None,
    )?;

    if created {
        println!("{}", "Created Dockerfile.railway file.".green());
    }

    Ok(())
}

/// Create a railway.toml file.
fn create_config_toml(base_dir: &Path) -> Result<()> {
    let created = write_template_file(base_dir.join("railway.toml"), RAILWAY_TOML, None)?;

    if created {
        println!("{}", "Created railway.toml file.".green());
    }

    Ok(())
}
