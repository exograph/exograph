// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{cmp::min, fs::create_dir_all, io::Write, path::PathBuf};

use anyhow::{anyhow, Ok, Result};
use futures::StreamExt;
use home::home_dir;
use indicatif::{ProgressBar, ProgressStyle};
use tempfile::NamedTempFile;

/// Download a file if it doesn't already exist in the cache.
///
/// Suitable for downloading large files where showing progress bar is useful.
///
/// # Arguments
/// - `url`: The URL to download from.
/// - `info_name`: The name of the file to display in the progress bar etc. (e.g. "Exograph AWS Distribution")
pub(super) async fn download_if_needed(url: &str, info_name: &str) -> Result<PathBuf> {
    let download_dir = cache_dir()?;

    // Download filename is the same as the last segment of the URL
    let download_file_name = url
        .split('/')
        .last()
        .ok_or(anyhow!("Failed to extract filename from URL"))?;
    let download_file_path = download_dir.join(download_file_name);

    if download_file_path.exists() {
        println!("Using a cached version of {info_name}");
        return Ok(download_file_path);
    }

    create_dir_all(&download_dir)?;

    let response = reqwest::get(url)
        .await
        .map_err(|e| anyhow!("Failed to fetch from '{}': {e}", &url))?;
    let content_length = response.content_length().unwrap_or(0);

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
    std::fs::rename(temp_downloaded_file.path(), &download_file_path)?;

    pb.finish_with_message("Downloaded!");

    Ok(download_file_path)
}

fn cache_dir() -> Result<PathBuf> {
    let current_version = env!("CARGO_PKG_VERSION");

    Ok(home_dir()
        .ok_or(anyhow!("Failed to resolve home directory"))?
        .join(".exograph")
        .join("cache")
        .join(current_version))
}
