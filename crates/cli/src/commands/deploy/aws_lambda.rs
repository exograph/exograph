// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{
    cmp::min,
    fs::{create_dir_all, File},
    io::Write,
    path::PathBuf,
};

use anyhow::{anyhow, Ok, Result};
use async_trait::async_trait;
use clap::{Arg, Command};
use colored::Colorize;
use futures::StreamExt;
use home::home_dir;
use indicatif::{ProgressBar, ProgressStyle};
use tempfile::NamedTempFile;

use crate::commands::{
    build::build,
    command::{get_required, CommandDefinition},
};

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
            .arg(
                Arg::new("app-name")
                    .help("The name of the application")
                    .short('a')
                    .long("app")
                    .required(true)
                    .num_args(1),
            )
    }

    async fn execute(&self, matches: &clap::ArgMatches) -> Result<()> {
        let download_file_name = "exograph-aws-lambda-linux-2023-x86_64.zip";
        let current_version = env!("CARGO_PKG_VERSION");
        let download_url = format!("https://github.com/exograph/exograph/releases/download/v{current_version}/{download_file_name}");
        let download_dir = home_dir()
            .ok_or(anyhow!("Failed to resolve home directory"))?
            .join(".exograph")
            .join("cache")
            .join(current_version);

        let downloaded_file_path = download_if_needed(
            &download_url,
            download_dir,
            download_file_name,
            "Exograph AWS Distribution",
        )
        .await?;

        let app_name: String = get_required(matches, "app-name")?;

        build(false).await?;

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

/// Download a file if it doesn't already exist.
///
/// Suitable for downloading large files where showing progress bar is useful.
///
/// # Arguments
/// - `url`: The URL to download from.
/// - `download_dir`: The directory to download to.
/// - `download_file_name`: The name of the file to download.
/// - `info_name`: The name of the file to display in the progress bar etc. (e.g. "Exograph AWS Distribution")
async fn download_if_needed(
    url: &str,
    download_dir: PathBuf,
    download_file_name: &str,
    info_name: &str,
) -> Result<PathBuf> {
    let download_file_path = download_dir.join(download_file_name);
    if download_file_path.exists() {
        println!("Using cached version of {info_name}");
        return Ok(download_file_path);
    }

    create_dir_all(&download_dir)?;

    let response = reqwest::get(url)
        .await
        .map_err(|e| anyhow!("Failed to fetch from '{}': {e}", &url))?;
    let content_length = response.content_length().unwrap_or(0);

    // Based on https://github.com/console-rs/indicatif/blob/main/examples/download.rs
    let pb = ProgressBar::new(content_length)
        .with_message(format!("Downloading {info_name}..."))
        .with_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{wide_bar:.cyan/blue}] {bytes}/{total_bytes}",
            )
            .unwrap()
            .progress_chars("#>-"),
        );

    let mut response_stream = response.bytes_stream();
    let mut downloaded_len: u64 = 0;
    let mut temp_downloaded_file = NamedTempFile::new_in(download_dir)?;

    // Download to a temporary file first
    while let Some(chunk) = response_stream.next().await {
        let chunk = chunk.map_err(|e| anyhow!("Failed to continue downloading {e}"))?;
        temp_downloaded_file
            .write(&chunk)
            .map_err(|e| anyhow!("Failed to write to the file {e}"))?;
        downloaded_len = min(downloaded_len + (chunk.len() as u64), content_length);
        pb.set_position(downloaded_len);
    }

    // Then move it to the final location. This avoids partially downloaded files.
    std::fs::rename(temp_downloaded_file.path(), &download_file_path)?;

    pb.finish_with_message("Downloaded!");

    Ok(download_file_path)
}
