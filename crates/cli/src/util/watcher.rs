// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{path::Path, time::Duration};

use anyhow::{anyhow, Context, Result};
use builder::error::ParserError;
use colored::Colorize;
use common::env_const::EXO_SERVER_PORT;
use core_model_builder::error::ModelBuildingError;
use futures::{future::BoxFuture, FutureExt};
use notify_debouncer_mini::notify::RecursiveMode;
use tokio::process::Child;

use crate::commands::{
    build::{build, BuildError},
    command::default_trusted_documents_dir,
};

/// Starts a watcher that will rebuild and serve model files with every change.
/// Takes a callback that will be called before the start of each server.
pub async fn start_watcher<'a, F>(
    root_path: &Path,
    server_port: Option<u32>,
    prestart_callback: F,
) -> Result<()>
where
    F: Fn() -> BoxFuture<'a, Result<()>>,
{
    // start watcher
    println!(
        "Watching the {} directory for changes...",
        if &root_path.display().to_string() == "." {
            "current".to_owned()
        } else {
            root_path.display().to_string()
        }
        .cyan()
        .bold(),
    );

    let canonical_root_path = root_path
        .canonicalize()
        .context("Failed to canonicalize path")?;

    let (watcher_tx, mut watcher_rx) = tokio::sync::mpsc::channel(1);
    let mut debouncer =
        notify_debouncer_mini::new_debouncer(Duration::from_millis(200), move |res| {
            let _ = watcher_tx.blocking_send(res);
        })?;
    debouncer
        .watcher()
        .watch(root_path, RecursiveMode::Recursive)?;

    // Given a path, determine if the model should be rebuilt and the server restarted.
    let should_restart = |path: &Path| -> bool {
        path.strip_prefix(&canonical_root_path)
            .map(|p| p.starts_with("src") || p.starts_with(default_trusted_documents_dir()))
            .unwrap_or(false)
    };

    let mut server = build_and_start_server(server_port, &prestart_callback).await?;

    loop {
        let server_death_event = if let Some(child) = server.as_mut() {
            child.wait().boxed()
        } else {
            // no server was spawned, so we should never fire this future
            std::future::pending().boxed()
        };

        let mut ctrl_c_receiver = crate::SIGINT.1.lock().await;
        let ctrl_c_event = ctrl_c_receiver.recv();

        let watcher_change = watcher_rx.recv();

        tokio::select! {
            maybe_events = watcher_change => {
                let Some(events) = maybe_events else {
                    break;  // quit if channel closed
                };

                if let Ok(events) = events {
                        if events.iter().map(|event| &event.path).any(|p| should_restart(p)) {
                            println!("\nChange detected, rebuilding and restarting...");
                            server = build_and_start_server(server_port, &prestart_callback).await?;
                        }
                    };
            }

            _ = ctrl_c_event => {
                // quit on CTRL-C
                break;
            }

            _ = server_death_event => {
                // server died for some reason, quit
                break;
            }
        }
    }

    Ok(())
}

pub(crate) async fn build_and_start_server<'a, F>(
    server_port: Option<u32>,
    prestart_callback: &F,
) -> Result<Option<Child>>
where
    F: Fn() -> BoxFuture<'a, Result<()>>,
{
    // Attempts to builds a exo_ir from the model and spawn a exo-server from it
    // - if the attempt succeeds, we will return a handle to the process in an Ok(Some(...))
    // - if the return value is an Err, this means that we have encountered an unrecoverable error, and so the
    //   watcher should exit.
    // - if the return value is an Ok(None), this mean that we have encountered some error, but it is not necessarily
    //   unrecoverable (the watcher should not exit)

    // precompute exo-server path and exo_ir file name
    let mut server_binary = std::env::current_exe()?;
    server_binary.set_file_name("exo-server");

    let build_result = build(false).await;

    match build_result {
        Ok(()) => {
            if let Err(e) = prestart_callback().await {
                println!("{} {}", "Error:".red().bold(), e.to_string().red().bold());
            }

            let mut command = tokio::process::Command::new(&server_binary);

            command.kill_on_drop(true);

            if let Some(port) = server_port {
                command.env(EXO_SERVER_PORT, port.to_string());
            }

            let child = command
                .spawn()
                .context("Failed to start exo-server")
                .map_err(|e| BuildError::UnrecoverableError(anyhow!(e)))?;

            Ok(Some(child))
        }

        // server encountered an unrecoverable error while building
        Err(BuildError::ParserError(ParserError::Generic(e)))
        | Err(BuildError::ParserError(ParserError::ModelBuildingError(
            ModelBuildingError::Generic(e),
        ))) => Err(anyhow!(e)),
        Err(BuildError::UnrecoverableError(e)) => Err(e),

        Err(BuildError::ParserError(ParserError::InvalidTrustedDocumentFormat(message))) => {
            println!("Error parsing trusted documents: {}", message.red().bold());
            Ok(None)
        }

        // server encountered a parser error (we don't need to exit the watcher)
        Err(BuildError::ParserError(_)) => Ok(None),
    }
}
