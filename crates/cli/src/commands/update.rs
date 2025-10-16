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
use std::{env, fs, path::Path, process::Command as ProcessCommand, sync::Arc};

use colored::Colorize;
use tempfile::tempdir_in;

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

            update_exograph(&latest_version).await?;

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

    if let Some(latest_version) = latest_version
        && current_version != latest_version
    {
        println!(
            "{}{}{}",
            "A new version (".yellow(),
            latest_version.green().bold(),
            ") of Exograph is available. Run `exo update` to install the new version.".yellow()
        );
    }

    Ok(())
}

async fn update_exograph(version: &str) -> anyhow::Result<()> {
    let target = get_target_triple()?;

    // Use same logic as install.sh: EXOGRAPH_INSTALL env var or default to ~/.exograph
    let install_root_dir = common::download::exo_install_root()?;

    // Use a lock file to prevent concurrent updates
    let lock_file_path = install_root_dir.join(".update.lock");
    let _file_lock = common::download::take_file_lock(&lock_file_path).await?;

    // Check if another process already updated while we waited for the lock
    let current_version = env!("CARGO_PKG_VERSION");
    if current_version == version {
        println!(
            "Already updated to version {} by another process!",
            version.green().bold()
        );
        return Ok(());
    }

    // Create temp directory parallel to install directory (for atomic rename)
    let temp_dir = tempdir_in(&install_root_dir)?.keep();

    // Download archive with checksum verification
    let zip_path = temp_dir.join(format!("exograph-{}.zip", target));
    let download_url = format!(
        "https://github.com/exograph/exograph/releases/download/v{}/exograph-{}.zip",
        version, target
    );
    let checksum_url = format!(
        "https://github.com/exograph/exograph/releases/download/v{}/exograph-{}.zip.sha256",
        version, target
    );
    common::download::download_with_checksum(&download_url, &checksum_url, "Exograph", &zip_path)
        .await?;

    println!("{}", "Extracting archive...".cyan());

    // Extract archive into temp directory
    common::download::extract_zip(&zip_path, &temp_dir)?;

    println!("{}", "Validating binary...".cyan());

    // Validate the main exo executable works
    let exe_extension = if cfg!(target_os = "windows") {
        ".exe"
    } else {
        ""
    };
    let main_exe = temp_dir.join(format!("exo{}", exe_extension));
    check_exe(&main_exe)?;

    println!("{}", "Installing update...".cyan());

    let install_bin_dir = install_root_dir.join("bin");

    // Atomically swap temp directory with install directory
    common::download::atomic_dir_swap(&temp_dir, &install_bin_dir)?;

    // Clean up the lock file
    fs::remove_file(&lock_file_path)?;

    println!("  {} Successfully updated!", "✓".green());

    Ok(())
}

/// Get the target triple for the current platform
fn get_target_triple() -> anyhow::Result<String> {
    let target = if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "aarch64-apple-darwin"
        } else {
            anyhow::bail!(
                "Intel Macs (x86_64) are no longer supported. \
                 Exograph now requires Apple Silicon (but you can build it yourself from sources)."
            );
        }
    } else if cfg!(target_os = "linux") {
        if cfg!(target_arch = "aarch64") {
            anyhow::bail!(
                "ARM64 Linux (aarch64) is not supported at this time. Please open an issue if you need this platform."
            );
        } else {
            "x86_64-unknown-linux-gnu"
        }
    } else if cfg!(target_os = "windows") {
        "x86_64-pc-windows-msvc"
    } else {
        anyhow::bail!("Unsupported platform");
    };

    Ok(target.to_string())
}

/// Validate that the extracted binary is functional
fn check_exe(exe_path: &Path) -> anyhow::Result<()> {
    let output = ProcessCommand::new(exe_path).arg("--version").output()?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to validate Exograph executable. \
             This may be because your OS is unsupported or the executable is corrupted"
        );
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
