// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{
    cmp::min,
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

use anyhow::{Result, anyhow};
use futures::StreamExt;
use home::home_dir;
use indicatif::{ProgressBar, ProgressStyle};
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

    let temp_download_dir = tempdir_in(exo_cache_root)?.into_path();

    let download_file_path = temp_download_dir.join(relative_cache_dir.replace('/', "_") + ".zip");

    download(url, info_name, &download_file_path).await?;

    let mut zip_file = zip::ZipArchive::new(File::open(&download_file_path)?)?;
    zip_file.extract(&temp_download_dir)?;

    fs::remove_file(&download_file_path)?;

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
    fs::create_dir_all(&download_dir)?;

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
    fs::rename(temp_downloaded_file.path(), &download_file_path)?;

    pb.finish_with_message("Downloaded!");

    Ok(())
}

pub fn exo_cache_root() -> Result<PathBuf> {
    Ok(home_dir()
        .ok_or(anyhow!("Failed to resolve home directory"))?
        .join(".exograph")
        .join("cache"))
}

async fn take_file_lock(lock_file_path: &PathBuf) -> Result<File> {
    use std::fs::OpenOptions;
    use std::time::Duration;

    // Create parent directory if it doesn't exist
    if let Some(parent) = lock_file_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .open(&lock_file_path)?;

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
