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
use clap::Command;
use exo_env::Environment;

use std::{fs::File, path::PathBuf, sync::Arc};

use rpc_introspection::{OpenRpcDocument, SchemaGeneration, to_rpc_document};

use crate::commands::{
    command::{CommandDefinition, default_model_file, output_arg, resolve_output_path},
    schema::util::create_system,
    util::use_ir_arg,
};
use crate::config::Config;

const OPENRPC_API_TITLE: &str = "Exograph RPC API";
const OPENRPC_API_VERSION: &str = "1.0.0";

pub(super) struct SchemaCommandDefinition {}

#[async_trait]
impl CommandDefinition for SchemaCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("schema")
            .about("Obtain the OpenRPC schema for the RPC API")
            .arg(output_arg().long_help(
                "Output file for the OpenRPC schema. Default: generated/rpc-schema.json",
            ))
            .arg(use_ir_arg())
    }

    async fn execute(
        &self,
        matches: &clap::ArgMatches,
        _config: &Config,
        _env: Arc<dyn Environment>,
    ) -> Result<()> {
        let use_ir: bool = matches.get_flag("use-ir");
        let model_path: PathBuf = default_model_file();

        let serialized_system = create_system(&model_path, None, use_ir).await?;

        let rpc_schema = serialized_system
            .rpc_schema
            .ok_or_else(|| anyhow!("No RPC schema found in the built system"))?;

        let rpc_document = to_rpc_document(&rpc_schema, SchemaGeneration::OpenRpc);
        let openrpc_document = OpenRpcDocument::new(OPENRPC_API_TITLE, OPENRPC_API_VERSION)
            .with_document(rpc_document);

        let output = resolve_output_path(matches, "rpc-schema.json")?;

        serde_json::to_writer_pretty(&mut File::create(&output)?, &openrpc_document)?;

        println!("OpenRPC schema written to {}", output.display());

        Ok(())
    }
}
