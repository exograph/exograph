// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::{anyhow, Result};
use builder::error::ParserError;
use clap::{ArgMatches, Command};

use std::error::Error;
use std::ffi::OsStr;
use std::fmt::Display;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::{fs::File, io::BufWriter};

use core_plugin_interface::interface::SubsystemBuilder;

use super::command::{get_required, model_file_arg, CommandDefinition};

pub struct BuildCommandDefinition {}

impl CommandDefinition for BuildCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("build")
            .about("Build exograph server binary")
            .arg(model_file_arg())
    }

    /// Build exograph server binary
    fn execute(&self, matches: &ArgMatches) -> Result<()> {
        let model: PathBuf = get_required(matches, "model")?;
        build(&model, true)?;

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
pub(crate) fn build_system_with_static_builders(model: &Path) -> Result<Vec<u8>, ParserError> {
    let static_builders: Vec<Box<dyn SubsystemBuilder>> = vec![
        Box::new(postgres_model_builder::PostgresSubsystemBuilder {}),
        Box::new(deno_model_builder::DenoSubsystemBuilder {}),
        Box::new(wasm_model_builder::WasmSubsystemBuilder {}),
    ];

    builder::build_system(model, static_builders)
}

/// Build exo_ir file
///
/// # Arguments
/// * `model` - exograph model path
/// * `system_start_time` - system start time. If specified, it will print a message indicated the time it took to build the model
/// * `print_message` - if true, it will print a message indicating the time it took to build the model. We need this
///                        to avoid printing the message when building the model through `exo serve`, where we don't want to print the message
///                        upon detecting changes
pub(crate) fn build(model: &Path, print_message: bool) -> Result<(), BuildError> {
    let serialized_system =
        build_system_with_static_builders(model).map_err(BuildError::ParserError)?;

    let exo_ir_file_name = {
        if let Some("exo") = model.extension().and_then(OsStr::to_str) {
            let mut filename = model.to_path_buf();
            filename.set_extension("exo_ir");
            filename
        } else {
            return Err(BuildError::UnrecoverableError(anyhow!(
                "{} is not a exo file",
                model.display()
            )));
        }
    };

    let mut out_file = BufWriter::new(File::create(&exo_ir_file_name).unwrap());
    out_file.write_all(&serialized_system).unwrap();

    if print_message {
        println!("Exograph IR file {} created", exo_ir_file_name.display());
        println!(
            "You can start the server with using the 'exo-server {}' command",
            exo_ir_file_name.display()
        );
    }

    Ok(())
}
