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
use clap::{Arg, Command, ValueEnum, builder::PossibleValue};
use exo_env::Environment;

use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::commands::{
    command::{CommandDefinition, default_model_file, get, output_arg},
    schema::util::create_system,
    util::use_ir_arg,
};
use crate::config::Config;

pub(super) struct SchemaCommandDefinition {}

#[async_trait]
impl CommandDefinition for SchemaCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("schema")
            .about("Obtain GraphQL schema")
            .arg(output_arg().long_help(
                "Output file for the introspection result. Default: generated/schema.graphql or generated/schema.json (depending on format)",
            ))
            .arg(
                Arg::new("format")
                    .long("format")
                    .short('f')
                    .value_parser(clap::builder::EnumValueParser::<SchemaFormat>::new())
                    .help("Output format. Default: graphql (sdl)")
                    .default_value("graphql"),
            )
            .arg(use_ir_arg())
    }

    /// Create a database schema from a exograph model
    async fn execute(
        &self,
        matches: &clap::ArgMatches,
        _config: &Config,
        _env: Arc<dyn Environment>,
    ) -> Result<()> {
        let use_ir: bool = matches.get_flag("use-ir");

        let model_path: PathBuf = default_model_file();

        let serialized_system = create_system(&model_path, None, use_ir).await?;

        let introspection_result = testing::get_introspection_result(serialized_system).await?;

        let format: SchemaFormat = match get(matches, "format") {
            Some(format) => format,
            None => SchemaFormat::Graphql,
        };

        match format {
            SchemaFormat::Json => {
                let output: PathBuf = match get(matches, "output") {
                    Some(output) => output,
                    None => {
                        fs::create_dir_all("generated")?;

                        Path::new("generated/schema.json").to_path_buf()
                    }
                };

                serde_json::to_writer_pretty(&mut File::create(output)?, &introspection_result)?;
            }
            SchemaFormat::Graphql => {
                let output: PathBuf = match get(matches, "output") {
                    Some(output) => output,
                    None => {
                        fs::create_dir_all("generated")?;

                        Path::new("generated/schema.graphql").to_path_buf()
                    }
                };

                let schema_string = introspection_util::schema_sdl(introspection_result).await?;

                File::create(output)?.write_all(schema_string.as_bytes())?;
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
enum SchemaFormat {
    Json,
    Graphql,
}

impl ValueEnum for SchemaFormat {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Json, Self::Graphql]
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        match self {
            Self::Json => Some(PossibleValue::new("json")),
            Self::Graphql => Some(PossibleValue::new("graphql")),
        }
    }
}
