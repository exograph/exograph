// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

#![cfg(not(target_family = "wasm"))]

use std::{
    cmp::min,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow};
use futures::StreamExt;
use home::home_dir;
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256};
use tempfile::{NamedTempFile, tempdir_in};

/// Download a file if it doesn't already exist.
///
/// Suitable for downloading large files where showing progress bar is useful.
///
/// Assumes that the file path will be ~/.exograph/cache/<exo_version>/<last-part-of-url>
///
/// # Arguments
/// - `url`: The URL to download from.
/// - `info_name`: The name of the file to display in the progress bar etc. (e.g. "Exograph AWS Distribution")
/// - `relative_cache_dir`: The relative path to the cache directory.
/// - `unzip`: Whether to unzip the file after downloading.
/// # Returns
/// The path to the downloaded file.
pub async fn download_file_if_needed(url: &str, info_name: &str) -> Result<PathBuf> {
    let exo_cache_root = exo_cache_root()?;
    let download_dir = exo_cache_root.join(env!("CARGO_PKG_VERSION"));
    // Download filename is the same as the last segment of the URL
    let download_file_name = url
        .split('/')
        .next_back()
        .ok_or(anyhow!("Failed to extract filename from URL"))?;
    let download_file_path = download_dir.join(download_file_name);

    if download_file_path.exists() {
        println!("Using a cached version of {info_name}");
        return Ok(download_file_path);
    }

    download(url, info_name, &download_file_path).await?;

    Ok(download_file_path)
}

/// Download and unzip a directory if it doesn't already exist
/// Assumes that zip file represents a directory
pub async fn download_dir_if_needed(
    url: &str,
    info_name: &str,
    relative_cache_dir: &str,
) -> Result<PathBuf> {
    let exo_cache_root = exo_cache_root()?;
    let download_dir = exo_cache_root.join(relative_cache_dir);

    if download_dir.exists() {
        return Ok(download_dir);
    }

    fs::create_dir_all(&exo_cache_root)?;

    // Use a lock file based on the target directory
    let file_lock_path = exo_cache_root.join(relative_cache_dir.replace('/', "_") + ".lock");

    let _file_lock = take_file_lock(&file_lock_path).await?;

    // Check once again if another process completed before we got the lock
    if download_dir.exists() {
        return Ok(download_dir);
    }

    let temp_download_dir = tempdir_in(exo_cache_root)?.keep();

    let download_file_path = temp_download_dir.join(relative_cache_dir.replace('/', "_") + ".zip");

    download(url, info_name, &download_file_path).await?;

    extract_zip(&download_file_path, &temp_download_dir)?;

    let download_dir_parent = download_dir
        .parent()
        .ok_or(anyhow!("Failed to get parent directory"))?;
    fs::create_dir_all(download_dir_parent)?;

    fs::rename(&temp_download_dir, &download_dir)?;

    fs::remove_file(&file_lock_path)?;

    Ok(download_dir)
}

async fn download(url: &str, info_name: &str, download_file_path: &PathBuf) -> Result<()> {
    let download_dir = download_file_path
        .parent()
        .ok_or(anyhow!("Failed to get parent directory"))?;
    fs::create_dir_all(download_dir)?;

    let response = reqwest::get(url)
        .await
        .map_err(|e| anyhow!("Failed to fetch from '{}': {e}", &url))?;
    let content_length = response.content_length().unwrap_or(0);

    println!("Downloading {info_name}...");

    // Based on https://github.com/console-rs/indicatif/blob/main/examples/download.rs
    let pb = ProgressBar::new(content_length)
        .with_message(format!("Downloading {info_name}..."))
        .with_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{wide_bar:.cyan/blue}] {bytes}/{total_bytes}",
            )
            .unwrap()
            .progress_chars("#>-"),
        );

    let mut response_stream = response.bytes_stream();
    let mut downloaded_len: u64 = 0;
    let mut temp_downloaded_file = NamedTempFile::new_in(download_dir)?;

    // Download to a temporary file first
    while let Some(chunk) = response_stream.next().await {
        let chunk = chunk.map_err(|e| anyhow!("Failed to continue downloading {e}"))?;
        temp_downloaded_file
            .write(&chunk)
            .map_err(|e| anyhow!("Failed to write to the file {e}"))?;
        downloaded_len = min(downloaded_len + (chunk.len() as u64), content_length);
        pb.set_position(downloaded_len);
    }

    // Then move it to the final location. This avoids partially downloaded files.
    fs::rename(temp_downloaded_file.path(), download_file_path)?;

    pb.finish_with_message("Downloaded!");

    Ok(())
}

/// Download a file and verify its SHA256 checksum
///
/// This function downloads both the target file and its .sha256 checksum file,
/// then verifies the checksum matches. If the checksum file is not available (404),
/// a warning is printed and verification is skipped.
///
/// # Arguments
/// * `url` - URL to download the file from
/// * `checksum_url` - URL to download the .sha256 checksum file from
/// * `info_name` - Display name for the progress bar
/// * `download_file_path` - Path to save the downloaded file
pub async fn download_with_checksum(
    url: &str,
    checksum_url: &str,
    info_name: &str,
    download_file_path: &PathBuf,
) -> Result<()> {
    use colored::Colorize;

    // Download the main file
    download(url, info_name, download_file_path).await?;

    println!("Verifying checksum...");

    // Try to download the checksum file
    let response = reqwest::get(checksum_url).await;

    let checksum_response = match response {
        Ok(resp) if resp.status().is_success() => resp,
        Ok(resp) if resp.status() == 404 => {
            println!(
                "{}",
                "Warning: Checksum file not found, skipping verification".yellow()
            );
            return Ok(());
        }
        Ok(resp) => {
            return Err(anyhow!(
                "Failed to download checksum file: HTTP {}",
                resp.status()
            ));
        }
        Err(e) => {
            return Err(anyhow!("Failed to download checksum file: {}", e));
        }
    };

    // Parse the checksum directly from response
    let checksum_bytes = checksum_response.bytes().await?;
    let checksum_text = String::from_utf8(checksum_bytes.to_vec())
        .map_err(|_| anyhow!("Invalid UTF-8 in checksum file"))?;

    let expected_checksum = checksum_text
        .split_whitespace()
        .next()
        .ok_or_else(|| anyhow!("Invalid checksum format"))?
        .to_string();

    // Verify the checksum
    verify_checksum(download_file_path, &expected_checksum)?;

    println!("{}", "  ✓ Checksum verified".green());

    Ok(())
}

/// Get the Exograph installation root directory
/// Uses EXOGRAPH_INSTALL env var if set, otherwise defaults to ~/.exograph
pub fn exo_install_root() -> Result<PathBuf> {
    match std::env::var("EXOGRAPH_INSTALL") {
        Ok(path) => Ok(PathBuf::from(path)),
        Err(_) => Ok(home_dir()
            .ok_or(anyhow!("Could not determine home directory"))?
            .join(".exograph")),
    }
}

pub fn exo_cache_root() -> Result<PathBuf> {
    Ok(exo_install_root()?.join("cache"))
}

/// Atomically swap a temporary directory with a target directory
///
/// This function performs an atomic directory swap with automatic backup and cleanup:
/// 1. If target exists, rename it to `target.old` (backup)
/// 2. Rename temp directory to target (atomic promotion)
/// 3. Delete the backup directory
///
/// This ensures that:
/// - The swap is atomic (both renames are atomic operations)
/// - If anything fails, the old directory can be manually recovered from `.old`
/// - No intermediate state where target directory is missing
///
/// # Arguments
/// * `temp_dir` - Path to the temporary directory with new content
/// * `target_dir` - Path to the target directory to replace
///
/// # Returns
/// * `Ok(())` on success
/// * `Err` if any operation fails
pub fn atomic_dir_swap(temp_dir: &PathBuf, target_dir: &PathBuf) -> Result<()> {
    let target_backup = target_dir
        .parent()
        .ok_or(anyhow!("Failed to get parent directory"))?
        .join(format!(
            "{}.old",
            target_dir
                .file_name()
                .ok_or(anyhow!("Invalid target directory name"))?
                .to_string_lossy()
        ));

    // Remove old backup if it exists from a previous swap
    if target_backup.exists() {
        fs::remove_dir_all(&target_backup)?;
    }

    // Atomically swap: target → backup, temp → target
    if target_dir.exists() {
        fs::rename(target_dir, &target_backup)?;
    }
    fs::rename(temp_dir, target_dir)?;

    // Clean up backup now that new version is successfully installed
    if target_backup.exists() {
        fs::remove_dir_all(&target_backup)?;
    }

    Ok(())
}

pub async fn take_file_lock(lock_file_path: &PathBuf) -> Result<File> {
    use std::fs::OpenOptions;
    use std::time::Duration;

    // Create parent directory if it doesn't exist
    if let Some(parent) = lock_file_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .open(lock_file_path)?;

    // Try to acquire exclusive lock with timeout
    let max_wait = Duration::from_secs(300); // 5 minutes
    let mut total_wait = Duration::from_secs(0);
    let wait_interval = Duration::from_millis(100);

    loop {
        match file.try_lock() {
            Ok(_) => return Ok(file),
            Err(fs::TryLockError::WouldBlock) => {
                println!(
                    "Waiting for file lock: {} (waited {:?})",
                    lock_file_path.display(),
                    total_wait
                );
                if total_wait >= max_wait {
                    return Err(anyhow!(
                        "Timeout waiting for file lock: {}",
                        lock_file_path.display()
                    ));
                }
                tokio::time::sleep(wait_interval).await;
                total_wait += wait_interval;
            }
            Err(fs::TryLockError::Error(e)) => {
                return Err(anyhow!(
                    "Failed to acquire file lock: {}: {}",
                    lock_file_path.display(),
                    e
                ));
            }
        }
    }
}

fn verify_checksum(file_path: &Path, expected: &str) -> Result<()> {
    let data = fs::read(file_path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let result = hasher.finalize();
    let actual = format!("{:x}", result);

    if actual != expected {
        anyhow::bail!(
            "Checksum verification failed!\nExpected: {}\nActual:   {}",
            expected,
            actual
        );
    }

    Ok(())
}

/// Extract a zip archive and remove the zip file
///
/// # Arguments
/// * `zip_path` - Path to the zip file to extract
/// * `target_dir` - Directory to extract files into
pub fn extract_zip(zip_path: &Path, target_dir: &Path) -> Result<()> {
    use std::fs::File;

    let zip_file = File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(zip_file)?;
    archive.extract(target_dir)?;
    fs::remove_file(zip_path)?;

    Ok(())
}
