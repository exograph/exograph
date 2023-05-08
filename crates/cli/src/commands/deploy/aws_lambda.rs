// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{
    env::current_exe,
    fs::{create_dir_all, File},
    path::PathBuf,
};

use anyhow::{anyhow, Ok, Result};
use clap::{Arg, Command};
use colored::Colorize;

use crate::commands::{
    build::build,
    command::{get_required, CommandDefinition},
};

/// The `deploy aws-lambda` command.
/// Creates a distributable zip file for AWS Lambda and provides instructions for deploying it.
///
/// Currently expects "aws-lambda-bootstrap" to be in the same directory as the exo executable.
/// To make this possible, run:
/// 1. docker/build.sh release
/// 2. docker cp $(docker create --name temp_container exo-server-aws-lambda:latest):/usr/src/app/bootstrap ./target/release/aws-lambda-bootstrap && docker rm temp_container
/// TODO: Revisit once we have a proper release process.
pub(super) struct AwsLambdaCommandDefinition {}

impl CommandDefinition for AwsLambdaCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("aws-lambda")
            .about("Deploy to AWS Lambda")
            .arg(
                Arg::new("app-name")
                    .help("The name of the application")
                    .short('a')
                    .long("app")
                    .required(true)
                    .num_args(1),
            )
    }

    fn execute(&self, matches: &clap::ArgMatches) -> Result<()> {
        let app_name: String = get_required(matches, "app-name")?;

        build(false)?;

        let aws_lambda_dir = PathBuf::from("target/aws-lambda");
        create_dir_all(aws_lambda_dir)?;

        println!(
            "{}",
            "Creating a new AWS Lambda function. This may take a few minutes."
                .purple()
                .italic()
        );
        create_function_zip()?;

        println!(
            "{}",
            "\nIf haven't already done so, run `aws configure` to set up access to your AWS account."
                .purple()
        );

        println!(
            "{}",
            "\nTo deploy the function for the first time, run:"
                .blue()
                .italic()
        );

        println!(
            "{}{}",
            "(cd ../.. && exo schema create) | psql ".blue(),
            "<your-postgres-url>".yellow(),
        );

        println!(
            "{}{}{}{}{}{}{}{}{}{}",
            "aws lambda create-function --function-name ".blue(),
            app_name.blue(),
            " --zip-file fileb://target/aws-lambda/function.zip --role arn:aws:iam::".blue(),
            "<account-id>".yellow(),
            ":role/".blue(),
            "<role>".yellow(),
            " --runtime=provided.al2 --handler=bootstrap".blue(),
            " --environment \"Variables={EXO_POSTGRES_URL=".blue(),
            "<your-postgres-url>".yellow(),
            "}\"".blue(),
        );

        println!(
            "{}",
            "\nTo deploy a new version of an existing app, run:"
                .green()
                .italic()
        );
        println!(
            "{}{}{}",
            "aws lambda update-function-code --function-name ".green(),
            app_name.green(),
            " --zip-file fileb://target/aws-lambda/function.zip".green(),
        );

        Ok(())
    }
}

/// Create a zip with the bootstrap executable and the compiled model.
fn create_function_zip() -> Result<()> {
    create_dir_all("target/aws-lambda")?;

    let zip_path = std::path::Path::new("target/aws-lambda/function.zip");
    let zip_file = std::fs::File::create(zip_path)?;

    let mut zip_writer = zip::ZipWriter::new(zip_file);

    let zip_options =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let bootstrap_location = current_exe()
        .unwrap()
        .parent()
        .ok_or(anyhow!("Failed to resolve installation directory"))?
        .join("aws-lambda-bootstrap");

    append_file(
        &mut zip_writer,
        "bootstrap",
        &bootstrap_location,
        zip_options.unix_permissions(0x777),
    )?;

    append_file(
        &mut zip_writer,
        "target/index.exo_ir",
        &PathBuf::from("target/index.exo_ir"),
        zip_options,
    )?;

    Ok(())
}

fn append_file(
    zip_writer: &mut zip::ZipWriter<File>,
    file_name: &str,
    file_path: &PathBuf,
    zip_options: zip::write::FileOptions,
) -> Result<()> {
    zip_writer.start_file(file_name, zip_options)?;

    let file = File::open(file_path)?;
    let mut reader = std::io::BufReader::new(file);

    std::io::copy(&mut reader, zip_writer)?;

    Ok(())
}
