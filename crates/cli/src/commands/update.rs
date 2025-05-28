// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use async_trait::async_trait;
use clap::{ArgMatches, Command};
use std::process::Command as ProcessCommand;

use colored::Colorize;

use crate::{commands::command::CommandDefinition, config::Config};

pub(crate) struct UpdateCommandDefinition {}

#[async_trait]
impl CommandDefinition for UpdateCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("update").about("Updates the Exograph version")
    }

    async fn execute(&self, _args: &ArgMatches, _config: &Config) -> anyhow::Result<()> {
        update_exograph_if_needed().await
    }

    async fn is_update_report_needed(&self) -> bool {
        false
    }
}

async fn update_exograph_if_needed() -> anyhow::Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    let latest_version = get_latest_version().await?;

    if current_version == latest_version {
        println!(
            "You are already on the latest version: {}",
            latest_version.green().bold()
        );
        return Ok(());
    }

    println!(
        "Updating to latest version: {}",
        latest_version.green().bold()
    );

    update_exograph().await?;

    println!(
        "Successfully updated to version: {}",
        latest_version.green().bold()
    );

    Ok(())
}

pub(crate) async fn report_update_needed() -> anyhow::Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    let latest_version = get_latest_version().await?;

    if current_version != latest_version {
        println!(
            "{}{}{}",
            "A new version (".yellow(),
            latest_version.green().bold(),
            ") of Exograph is available. Run `exo update` to update.".yellow()
        );
    }

    Ok(())
}

async fn update_exograph() -> anyhow::Result<()> {
    let status = if cfg!(target_os = "windows") {
        ProcessCommand::new("powershell")
                .args([
                    "-Command",
                    "irm https://raw.githubusercontent.com/exograph/exograph/main/installer/install.ps1 | iex",
                ])
                .status()?
    } else {
        ProcessCommand::new("sh")
                .args([
                    "-c",
                    "curl -fsSL https://raw.githubusercontent.com/exograph/exograph/main/installer/install.sh | sh",
                ])
                .status()?
    };

    if !status.success() {
        anyhow::bail!("Failed to update Exograph");
    }

    Ok(())
}

async fn get_latest_version() -> anyhow::Result<String> {
    let response = reqwest::Client::new()
        .get("https://api.github.com/repos/exograph/exograph/releases/latest")
        .header("User-Agent", "cli")
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;

    let version = response
        .get("tag_name")
        .ok_or(anyhow::anyhow!("No version found"))?
        .as_str()
        .ok_or(anyhow::anyhow!("Unable to parse version"))?
        .strip_prefix("v")
        .ok_or(anyhow::anyhow!("Invalid version format"))?;

    Ok(version.to_string())
}
