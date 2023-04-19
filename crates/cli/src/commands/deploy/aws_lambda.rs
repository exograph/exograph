use std::{
    env::current_exe,
    fs::{create_dir_all, File},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Ok, Result};
use clap::{Arg, Command};
use colored::Colorize;

use crate::commands::{
    build::build,
    command::{get_required, model_file_arg, CommandDefinition},
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
            .arg(model_file_arg())
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
        let model: PathBuf = get_required(matches, "model")?;
        let app_name: String = get_required(matches, "app-name")?;

        build(&model, false)?;

        // Canonicalize the model path so that when presented with just "filename.exo", we can still
        // get the directory that it's in.
        let model_path = model.canonicalize()?;
        let model_dir = model_path.parent().unwrap();
        let aws_lambda_dir = model_dir.join("aws-lambda");
        create_dir_all(aws_lambda_dir)?;

        println!(
            "{}",
            "Creating a new AWS Lambda function. This may take a few minutes."
                .purple()
                .italic()
        );
        create_function_zip(&model_path)?;

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
            "{}{}{}{}",
            "exo schema create ".blue(),
            model.to_str().unwrap().blue(),
            " | psql ".blue(),
            "<your-postgres-url>".yellow(),
        );

        println!(
            "{}{}{}{}{}{}{}{}{}{}",
            "aws lambda create-function --function-name ".blue(),
            app_name.blue(),
            " --zip-file fileb://aws-lambda/function.zip --role arn:aws:iam::".blue(),
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
            " --zip-file fileb://aws-lambda/function.zip".green(),
        );

        Ok(())
    }
}

/// Create a zip with the bootstrap executable and the compiled model.
fn create_function_zip(model_path: &Path) -> Result<()> {
    let zip_path = std::path::Path::new("aws-lambda/function.zip");
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

    let exo_ir_location = &(*model_path).with_extension("exo_ir");
    append_file(
        &mut zip_writer,
        "index.exo_ir",
        exo_ir_location,
        zip_options,
    )?;

    Ok(())
}

fn append_file(
    zip_writer: &mut zip::ZipWriter<File>,
    file_name: &str,
    file_path: &Path,
    zip_options: zip::write::FileOptions,
) -> Result<()> {
    zip_writer.start_file(file_name, zip_options)?;

    let file = File::open(file_path)?;
    let mut reader = std::io::BufReader::new(file);

    std::io::copy(&mut reader, zip_writer)?;

    Ok(())
}
