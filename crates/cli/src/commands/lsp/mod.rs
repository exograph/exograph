// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::config::Config;
use anyhow::Result;
use async_trait::async_trait;
use clap::{Arg, ArgMatches, Command};

use super::command::CommandDefinition;

mod server;

pub struct LspCommandDefinition {}

#[async_trait]
impl CommandDefinition for LspCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("lsp").about("Start the LSP server").arg(
            Arg::new("stdio")
                .help("Use stdio for the LSP server")
                .long("stdio")
                .required(false)
                .num_args(0),
        )
    }

    /// Build exograph server binary
    async fn execute(&self, _matches: &ArgMatches, _config: &Config) -> Result<()> {
        server::start().await?;

        Ok(())
    }
}
