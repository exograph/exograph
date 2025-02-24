// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use builder::error::ParserError;
use clap::{ArgMatches, Command};
use core_plugin_interface::interface::SubsystemBuilder;
use core_plugin_shared::serializable_system::SerializableSystem;
use core_plugin_shared::system_serializer::SystemSerializer;

use std::error::Error;
use std::fmt::Display;
use std::fs::create_dir_all;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::{fs::File, io::BufWriter};

use crate::commands::command::default_model_file;
use crate::config::Config;
use crate::util::watcher::execute_after_scripts;
use crate::util::watcher::execute_before_scripts;

use super::command::default_trusted_documents_dir;
use super::command::ensure_exo_project_dir;
use super::command::CommandDefinition;

pub struct BuildCommandDefinition {}

#[async_trait]
impl CommandDefinition for BuildCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("build").about("Build exograph server binary")
    }

    /// Build exograph server binary
    async fn execute(&self, _matches: &ArgMatches, config: &Config) -> Result<()> {
        build(true, config).await?;

        Ok(())
    }
}

#[derive(Debug)]
pub enum BuildError {
    ParserError(ParserError),
    UnrecoverableError(anyhow::Error),
}

impl Error for BuildError {}

impl Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            BuildError::ParserError(e) => writeln!(f, "Parser error: {e}"),
            BuildError::UnrecoverableError(e) => writeln!(f, "{e}"),
        }
    }
}

/// Use statically linked builder to avoid dynamic loading for the CLI
pub(crate) async fn build_system_with_static_builders(
    model: &Path,
    trusted_documents_dir: Option<&Path>,
) -> Result<SerializableSystem, ParserError> {
    let static_builders: Vec<Box<dyn SubsystemBuilder + Send + Sync>> = vec![
        Box::new(postgres_builder::PostgresSubsystemBuilder::default()),
        Box::new(deno_builder::DenoSubsystemBuilder::default()),
        Box::new(wasm_builder::WasmSubsystemBuilder::default()),
    ];

    builder::build_system(model, trusted_documents_dir, static_builders).await
}

/// Build exo_ir file
///
/// # Arguments
/// * `model` - exograph model file path
/// * `output` - output file path
/// * `print_message` - if true, it will print a message indicating the time it took to build the model. We need this
///                        to avoid printing the message when building the model through `exo dev`, where we don't want to print the message
///                        upon detecting changes
pub(crate) async fn build(print_message: bool, config: &Config) -> Result<(), BuildError> {
    ensure_exo_project_dir(&PathBuf::from("."))?;

    execute_before_scripts(config).map_err(BuildError::UnrecoverableError)?;

    let model: PathBuf = default_model_file();
    let trusted_documents_dir = default_trusted_documents_dir();

    let serialized_system = build_system_with_static_builders(&model, Some(&trusted_documents_dir))
        .await
        .map_err(BuildError::ParserError)?;

    let exo_ir_file_name = PathBuf::from("target/index.exo_ir");
    create_dir_all("target").map_err(|e| {
        BuildError::UnrecoverableError(anyhow!("Could not create the target directory: {}", e))
    })?;

    let mut out_file = BufWriter::new(File::create(&exo_ir_file_name).unwrap());
    let serialized = SystemSerializer::serialize(&serialized_system)
        .map_err(|e| BuildError::ParserError(e.into()))?;
    out_file.write_all(&serialized).unwrap();

    execute_after_scripts(config).map_err(BuildError::UnrecoverableError)?;

    if print_message {
        println!("Exograph IR file {} created", exo_ir_file_name.display());
        println!("You can start the server with using the 'exo-server' command");
    }

    Ok(())
}
