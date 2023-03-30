use std::{path::Path, time::Duration};

use anyhow::{anyhow, Context, Result};
use builder::error::ParserError;
use core_model_builder::error::ModelBuildingError;
use futures::{future::BoxFuture, FutureExt};
use notify_debouncer_mini::notify::RecursiveMode;

use crate::commands::build::{build, BuildError};

/// Starts a watcher that will rebuild and serve model files with every change.
/// Takes a callback that will be called before the start of each server.
pub async fn start_watcher<'a, F>(
    model_path: &Path,
    server_port: Option<u32>,
    prestart_callback: F,
) -> Result<()>
where
    F: Fn() -> BoxFuture<'a, Result<()>>,
{
    let absolute_path = model_path
        .canonicalize()
        .map_err(|e| anyhow!("Could not find {}: {}", model_path.to_string_lossy(), e))?;
    let parent_dir = absolute_path.parent().ok_or_else(|| {
        anyhow!(
            "Could not get parent directory of {}",
            model_path.to_string_lossy()
        )
    })?;

    // start watcher
    println!(
        "{} {}",
        ansi_term::Color::Blue.bold().paint("Watching:"),
        ansi_term::Color::Cyan
            .bold()
            .paint(&parent_dir.display().to_string())
    );

    let (watcher_tx, mut watcher_rx) = tokio::sync::mpsc::channel(1);
    let mut debouncer =
        notify_debouncer_mini::new_debouncer(Duration::from_millis(200), None, move |res| {
            let _ = watcher_tx.blocking_send(res);
        })?;
    debouncer
        .watcher()
        .watch(parent_dir, RecursiveMode::Recursive)?;

    // precompute exo-server path and exo_ir file name
    let mut server_binary = std::env::current_exe()?;
    server_binary.set_file_name("exo-server");
    let exo_ir_file_name = format!("{}_ir", model_path.to_str().unwrap());

    // Given a path, determine if the model should be rebuilt and the server restarted.
    fn should_restart(path: &Path) -> bool {
        !matches!(path.extension().and_then(|e| e.to_str()), Some("exo_ir"))
    }

    // Attempts to builds a exo_ir from the model and spawn a exo-server from it
    // - if the attempt succeeds, we will return a handle to the process in an Ok(Some(...))
    // - if the return value is an Err, this means that we have encountered an unrecoverable error, and so the
    //   watcher should exit.
    // - if the return value is an Ok(None), this mean that we have encountered some error, but it is not necessarily
    //   unrecoverable (the watcher should not exit)
    let build_and_start_server = &|| async {
        let build_result = build(&absolute_path, None, false);

        match build_result {
            Ok(()) => {
                if let Err(e) = prestart_callback().await {
                    println!(
                        "{} {}",
                        ansi_term::Color::Red.bold().paint("Error:"),
                        ansi_term::Color::Red.bold().paint(e.to_string())
                    );
                }

                let mut command = tokio::process::Command::new(&server_binary);

                command.args([&exo_ir_file_name]);
                command.kill_on_drop(true);

                if let Some(port) = server_port {
                    command.env("EXO_SERVER_PORT", port.to_string());
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

            // server encountered a parser error (we don't need to exit the watcher)
            Err(BuildError::ParserError(_)) => Ok(None),
        }
    };

    let mut server = build_and_start_server().await?;

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
                            println!("Change detected, rebuilding and restarting...");
                            server = build_and_start_server().await?;
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
