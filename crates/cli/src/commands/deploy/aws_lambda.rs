use std::{
    fs::{create_dir_all, File},
    io::{BufRead, Write},
    path::{Path, PathBuf},
};

use ansi_term::Color;
use anyhow::{anyhow, Result};
use clap::{Arg, ArgAction, Command};

use crate::commands::{
    build::build,
    command::{get, get_required, model_file_arg, CommandDefinition},
};

pub(super) struct AwsLambdaCommandDefinition {}

impl CommandDefinition for AwsLambdaCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("aws-lambda")
            .about("Deploy to AWS Lambda")
            .arg(model_file_arg())
            .arg(
                Arg::new("app-name")
                    .help("The name of the Fly.io application to deploy to")
                    .short('a')
                    .long("app")
                    .required(true)
                    .num_args(1),
            )
            .arg(
                Arg::new("version")
                    .help("The version of application (Dockerfile will use this as tag)")
                    .short('v')
                    .long("version")
                    .required(false)
                    .default_value("latest")
                    .num_args(1),
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
    }

    fn execute(&self, matches: &clap::ArgMatches) -> Result<()> {
        let model: PathBuf = get_required(matches, "model")?;
        let app_name: String = get_required(matches, "app-name")?;
        let version: String = get_required(matches, "version")?;
        let envs: Option<Vec<String>> = matches.get_many("env").map(|env| env.cloned().collect());
        let env_file: Option<PathBuf> = get(matches, "env-file");

        build(&model, false)?;

        // Canonicalize the model path so that when presented with just "filename.exo", we can still
        // get the directory that it's in.
        let model_path = model.canonicalize()?;
        let model_dir = model_path.parent().unwrap();
        let aws_lambda_dir = model_dir.join("aws-lambda");
        create_dir_all(&aws_lambda_dir)?;

        Ok(())
    }
}
