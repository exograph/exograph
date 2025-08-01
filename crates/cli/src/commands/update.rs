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
use exo_env::Environment;
use std::{process::Command as ProcessCommand, sync::Arc};

use colored::Colorize;

use common::env_processing::EnvProcessing;

use crate::{commands::command::CommandDefinition, config::Config};

pub(crate) struct UpdateCommandDefinition {}

#[async_trait]
impl CommandDefinition for UpdateCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("update").about("Updates the Exograph version")
    }

    fn env_processing(&self, _env: &dyn Environment) -> EnvProcessing {
        EnvProcessing::DoNotProcess
    }

    async fn execute(
        &self,
        _args: &ArgMatches,
        _config: &Config,
        _env: Arc<dyn Environment>,
    ) -> anyhow::Result<()> {
        let current_version = env!("CARGO_PKG_VERSION");
        let latest_version = get_latest_version(true).await?;

        if let Some(latest_version) = latest_version {
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
        } else {
            println!("{}", "Unable to check for updates".yellow());
        }

        Ok(())
    }

    async fn is_update_report_needed(&self) -> bool {
        false
    }
}

pub(crate) async fn report_update_needed(env: &dyn Environment) -> anyhow::Result<()> {
    let skip_update_check = env.enabled("EXO_SKIP_UPDATE_CHECK", false)?;

    if skip_update_check {
        return Ok(());
    }

    let current_version = env!("CARGO_PKG_VERSION");
    let latest_version = get_latest_version(false).await?;

    if let Some(latest_version) = latest_version {
        if current_version != latest_version {
            println!(
                "{}{}{}",
                "A new version (".yellow(),
                latest_version.green().bold(),
                ") of Exograph is available. Run `exo update` to install the new version.".yellow()
            );
        }
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

/// Get the latest version of Exograph from GitHub.
///
/// If `fail_on_network_error` is true, this function will return an error if the request fails.
/// If `fail_on_network_error` is false, this function will return `None` if the request fails.
///
/// The error/None behavior allows normal operation of commands such as `exo build` when working
/// offline.
async fn get_latest_version(fail_on_network_error: bool) -> anyhow::Result<Option<String>> {
    let response = reqwest::Client::new()
        .get("https://api.github.com/repos/exograph/exograph/releases/latest")
        .header("User-Agent", "cli")
        .send()
        .await;

    let response = match response {
        Ok(response) => response,
        Err(e) => {
            if fail_on_network_error {
                tracing::error!("Failed to make a network call: {e}");
                anyhow::bail!("Failed to get latest version due to a network error");
            } else {
                return Ok(None);
            }
        }
    };

    let status = response.status();

    if !status.is_success() {
        if fail_on_network_error {
            anyhow::bail!("Failed to get latest version due to a network error (status: {status})");
        } else {
            return Ok(None);
        }
    }

    let json = response.json::<serde_json::Value>().await?;

    let version = json
        .get("tag_name")
        .ok_or(anyhow::anyhow!("No version found"))?
        .as_str()
        .ok_or(anyhow::anyhow!("Unable to parse version"))?
        .strip_prefix("v")
        .ok_or(anyhow::anyhow!("Invalid version format"))?;

    Ok(Some(version.to_string()))
}
