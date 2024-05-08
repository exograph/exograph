// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, Write},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use clap::{Arg, ArgAction, Command};
use colored::Colorize;

use crate::commands::{
    build::build,
    command::{get, CommandDefinition},
};
use common::env_const::EXO_POSTGRES_URL;

use super::util::{app_name_arg, app_name_from_args, write_template_file};

pub(super) struct FlyCommandDefinition {}

#[async_trait]
impl CommandDefinition for FlyCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("fly")
            .about("Deploy to Fly.io")
            .arg(
                app_name_arg(),
            )
            .arg(
                Arg::new("env")
                    .help("Environment variables to pass to the application (e.g. -e KEY=VALUE). May be specified multiple times.")
                    .action(ArgAction::Append) // To allow multiple --env flags ("-e k1=v1 -e k2=v2")
                    .short('e')
                    .long("env")
                    .num_args(1),
            )
            .arg(
                Arg::new("env-file").help("Path to a file containing environment variables to pass to the application")
                    .long("env-file")
                    .required(false)
                    .value_parser(clap::value_parser!(PathBuf))
                    .num_args(1),
            )
            .arg(
                Arg::new("use-fly-db")
                    .help("Use database provided by Fly.io")
                    .required(false)
                    .long("use-fly-db")
                    .num_args(0),
            )
    }

    /// Create a fly.toml file, a Dockerfile, and build the docker image. Then provide instructions
    /// on how to deploy the app to Fly.io.
    async fn execute(&self, matches: &clap::ArgMatches) -> Result<()> {
        let app_name: String = app_name_from_args(matches);
        let envs: Option<Vec<String>> = matches.get_many("env").map(|env| env.cloned().collect());
        let env_file: Option<PathBuf> = get(matches, "env-file");
        let use_fly_db: bool = matches.get_flag("use-fly-db");

        build(false).await?; // Build the exo_ir file

        let current_dir = std::env::current_dir()?;

        create_fly_toml(&current_dir, &app_name, &env_file, &envs)?;
        create_dockerfile(&current_dir, use_fly_db)?;

        println!(
            "\n{}\n",
            "To deploy to Fly.io, run the following commands:".green()
        );

        println!(
            "\t{} {}",
            "flyctl auth login".blue(),
            "(If you haven't already done so)".purple()
        );

        println!("\t{}", format!("flyctl apps create {}", app_name).blue());
        println!(
            "\n\tSet up JWT by running {} of the following: ",
            "either".bold()
        );
        println!(
            "\t{}{}",
            format!("flyctl secrets set --app {} EXO_JWT_SECRET=", app_name).blue(),
            "<your-jwt-secret>".yellow()
        );
        println!(
            "\t{}{}",
            format!("flyctl secrets set --app {} EXO_OIDC_URL=", app_name,).blue(),
            "<your-oidc-url>".yellow()
        );
        println!("\n\tSet up the database: ");

        if use_fly_db {
            println!(
                "\t{}",
                format!("flyctl postgres create --name {}-db", app_name).blue()
            );
            println!(
                "\t{}",
                format!("flyctl postgres attach --app {} {}-db", app_name, app_name).blue()
            );
        } else {
            println!(
                "\t{}{}{}",
                format!("flyctl secrets set --app {} DATABASE_URL=\"", app_name).blue(),
                "<your-postgres-url>".yellow(),
                "\"".blue()
            );
        }

        println!("\n\tDeploy the app: ");

        println!(
            "\t{}",
            r#"flyctl console --dockerfile Dockerfile.fly.builder -C "/srv/deploy.sh" --env=FLY_API_TOKEN=$(flyctl auth token)"#.blue(),
        );

        Ok(())
    }
}

static FLY_TOML: &str = include_str!("../templates/fly.toml");
static DOCKERFILE: &str = include_str!("../templates/Dockerfile.fly");
static DOCKERFILE_BUILDER: &str = include_str!("../templates/Dockerfile.fly.builder");

fn create_dockerfile(fly_dir: &Path, use_fly_db: bool) -> Result<()> {
    {
        let pg_key_val = format!("EXO_POSTGRES_URL={}", EXO_POSTGRES_URL);
        let substitute =
            use_fly_db.then(|| HashMap::from([("<<<EXTRA_ENV>>>", pg_key_val.as_str())]));

        let created = write_template_file(fly_dir.join("Dockerfile.fly"), DOCKERFILE, substitute)?;

        if created {
            println!(
                "{}",
                "Created Dockerfile.fly. You can edit this file to customize the deployment such as installing additional dependencies."
                    .green()
            );
        }
    }

    {
        write_template_file(
            fly_dir.join("Dockerfile.fly.builder"),
            DOCKERFILE_BUILDER,
            None,
        )?;
    }

    Ok(())
}

/// Create a fly.toml file.
/// Replaces the placeholders in the template with the app name and image tag
/// as well as the environment variables.
fn create_fly_toml(
    fly_dir: &Path,
    app_name: &str,
    env_file: &Option<PathBuf>,
    envs: &Option<Vec<String>>,
) -> Result<()> {
    let fly_toml_file_path = fly_dir.join("fly.toml");

    if fly_toml_file_path.exists() {
        println!(
            "{}",
            "fly.toml file already exists. To regenerate, remove the existing file. Skipping..."
                .purple()
        );
        return Ok(());
    }

    let fly_toml_content = FLY_TOML.replace("<<<APP_NAME>>>", app_name);

    let mut accumulated_env = String::new();

    // First process the env file, if any (so that explicit --env overrides the env file values)
    if let Some(env_file) = &env_file {
        let env_file = File::open(env_file).map_err(|e| {
            anyhow!(
                "Failed to open env file '{}': {}",
                env_file.to_str().unwrap(),
                e
            )
        })?;
        let reader = std::io::BufReader::new(env_file);
        for line in reader.lines() {
            let line = line?;
            accumulate_env(&mut accumulated_env, &line)?;
        }
    }

    for env in envs.iter().flatten() {
        accumulate_env(&mut accumulated_env, env)?;
    }

    let mut fly_toml_file = File::create(fly_toml_file_path)?;
    let fly_toml_content = fly_toml_content.replace("<<<ENV_VARS>>>", &accumulated_env);
    fly_toml_file.write_all(fly_toml_content.as_bytes())?;

    println!(
        "{}",
        "Created fly.toml file. You can edit this file to customize the deployment such as setting the deployment region."
            .green()
    );

    Ok(())
}

fn accumulate_env(envs: &mut String, env: &str) -> Result<()> {
    if env.starts_with('#') {
        return Ok(());
    }
    let parts: Vec<_> = env.split('=').collect();
    if parts.len() != 2 {
        return Err(anyhow!(
            "Invalid env specified. Must be in the form of KEY=VALUE"
        ));
    }
    let key = parts[0].to_string();
    let value = parts[1].to_string();
    envs.push_str(&format!("{}=\"{}\"\n", key, value));

    Ok(())
}
