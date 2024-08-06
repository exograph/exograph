// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::Result;
use async_trait::async_trait;
use clap::Command;

use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

use crate::commands::{
    command::{default_model_file, get, output_arg, CommandDefinition},
    schema::util::create_system,
    util::use_ir_arg,
};

pub(super) struct SchemaCommandDefinition {}

#[async_trait]
impl CommandDefinition for SchemaCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("schema")
            .about("Obtain GraphQL schema")
            .arg(output_arg().long_help(
                "Output file for the introspection result. Default: generated/schema.json",
            ))
            .arg(use_ir_arg())
    }

    /// Create a database schema from a exograph model
    async fn execute(&self, matches: &clap::ArgMatches) -> Result<()> {
        let use_ir: bool = matches.get_flag("use-ir");

        let model_path: PathBuf = default_model_file();

        let output: PathBuf = match get(matches, "output") {
            Some(output) => output,
            None => {
                fs::create_dir_all("generated")?;
                let output = Path::new("generated/schema.json").to_path_buf();
                output
            }
        };

        let serialized_system = create_system(&model_path, None, use_ir).await?;

        let introspection_result = testing::get_introspection_result(serialized_system).await?;

        serde_json::to_writer_pretty(&mut File::create(output)?, &introspection_result)?;

        Ok(())
    }
}
