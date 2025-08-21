// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{
    fs::{File, create_dir_all},
    path::PathBuf,
    sync::Arc,
};

use anyhow::{Ok, Result};
use async_trait::async_trait;
use clap::Command;
use colored::Colorize;
use exo_env::Environment;

use crate::commands::{build::build, command::CommandDefinition};
use crate::config::Config;

use common::download::download_if_needed;

use super::{util::app_name_arg, util::app_name_from_args};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The `deploy aws-lambda` command.
///
/// Creates a distributable zip file for AWS Lambda and provides instructions for deploying it.
///
/// This command:
/// - Builds the exo_ir file.
/// - Downloads the distribution file build by CI (and caches it in ~/.exograph/cache/<version>).
/// - Creates target/aws/function.zip with `bootstrap` from the distribution with the exo_ir file.
/// - Prints instructions for users to carry out.
pub(super) struct AwsLambdaCommandDefinition {}

#[async_trait]
impl CommandDefinition for AwsLambdaCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("aws-lambda")
            .about("Deploy to AWS Lambda")
            .arg(app_name_arg())
    }

    async fn execute(
        &self,
        matches: &clap::ArgMatches,
        config: &Config,
        _env: Arc<dyn Environment>,
    ) -> Result<()> {
        let download_file_name = "exograph-aws-lambda-linux-2023-x86_64.zip";
        let download_url = format!(
            "https://github.com/exograph/exograph/releases/download/v{CURRENT_VERSION}/{download_file_name}"
        );

        let downloaded_file_path =
            download_if_needed(&download_url, "Exograph AWS Distribution", None, false).await?;

        let app_name: String = app_name_from_args(matches);

        build(false, config).await?;

        let aws_lambda_dir = PathBuf::from("target/aws-lambda");
        create_dir_all(aws_lambda_dir)?;

        println!(
            "{}",
            "Creating a new AWS Lambda function.".purple().italic()
        );
        create_function_zip(downloaded_file_path)?;

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
            "exo schema migrate --apply-to-database --database ".blue(),
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
            " --runtime=provided.al2023 --handler=bootstrap".blue(),
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
fn create_function_zip(distribution_zip_path: PathBuf) -> Result<()> {
    let mut distribution_zip_file = zip::ZipArchive::new(File::open(distribution_zip_path)?)?;

    let zip_path = std::path::Path::new("target/aws-lambda/function.zip");
    let zip_file = std::fs::File::create(zip_path)?;

    let mut zip_writer = zip::ZipWriter::new(zip_file);

    let zip_options =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    zip_writer.raw_copy_file(distribution_zip_file.by_name("bootstrap")?)?;

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
