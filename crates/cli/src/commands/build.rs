// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use builder::RealFileSystem;
use builder::error::ParserError;
use clap::{ArgMatches, Command};
use common::env_processing::EnvProcessing;
use core_model_builder::plugin::BuildMode;
use core_plugin_interface::interface::SubsystemBuilder;
use core_plugin_shared::profile::SchemaProfiles;
use core_plugin_shared::serializable_system::SerializableSystem;
use core_plugin_shared::system_serializer::SystemSerializer;
use exo_env::Environment;

use std::error::Error;
use std::fmt::Display;
use std::fs::create_dir_all;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::{fs::File, io::BufWriter};

use crate::commands::command::default_model_file;
use crate::config::Config;
use crate::config::WatchStage;
use crate::util::watcher::execute_scripts;

use super::command::CommandDefinition;
use super::command::default_trusted_documents_dir;
use super::command::ensure_exo_project_dir;

pub struct BuildCommandDefinition {}

#[async_trait]
impl CommandDefinition for BuildCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("build").about("Build exograph server binary")
    }

    /// Build command does not process env files.
    fn env_processing(&self, _env: &dyn Environment) -> EnvProcessing {
        EnvProcessing::DoNotProcess
    }

    /// Build exograph server binary
    async fn execute(
        &self,
        _matches: &ArgMatches,
        config: &Config,
        _env: Arc<dyn Environment>,
    ) -> Result<()> {
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
    schema_profiles: Option<SchemaProfiles>,
) -> Result<SerializableSystem, ParserError> {
    let static_builders: Vec<Box<dyn SubsystemBuilder + Send + Sync>> = vec![
        Box::new(postgres_builder::PostgresSubsystemBuilder::default()),
        Box::new(deno_builder::DenoSubsystemBuilder::default()),
        Box::new(wasm_builder::WasmSubsystemBuilder::default()),
    ];

    builder::build_system(
        model,
        &RealFileSystem,
        trusted_documents_dir,
        schema_profiles,
        static_builders,
        BuildMode::Build,
    )
    .await
}

/// Build exo_ir file
///
/// # Arguments
/// * `model` - exograph model file path
/// * `output` - output file path
/// * `print_message` - if true, it will print a message indicating the time it took to build the model. We need this
///   to avoid printing the message when building the model through `exo dev`, where we don't want to print the message
///   upon detecting changes
pub(crate) async fn build(print_message: bool, config: &Config) -> Result<(), BuildError> {
    ensure_exo_project_dir(&PathBuf::from("."))?;

    let model: PathBuf = default_model_file();
    let trusted_documents_dir = default_trusted_documents_dir();

    let serialized_system =
        build_system_with_static_builders(&model, Some(&trusted_documents_dir), config.mcp.clone())
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

    execute_scripts(config, &WatchStage::Build).map_err(BuildError::UnrecoverableError)?;

    if print_message {
        println!("Exograph IR file {} created", exo_ir_file_name.display());
        println!("You can start the server with using the 'exo-server' command");
    }

    Ok(())
}
