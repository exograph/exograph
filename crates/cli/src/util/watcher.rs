use std::{
    path::Path,
    process::Child,
    sync::{
        atomic::Ordering,
        mpsc::{channel, RecvTimeoutError},
    },
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use builder::error::ParserError;
use core_model_builder::error::ModelBuildingError;
use notify::{DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};

use crate::commands::build::{build, BuildError};

/// Starts a watcher that will rebuild and serve model files with every change.
/// Takes a callback that will be called before the start of each server.
pub fn start_watcher<F>(
    model_path: &Path,
    server_port: Option<u32>,
    prestart_callback: F,
) -> Result<()>
where
    F: Fn() -> Result<()>,
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

    println!("Watching: {:?}", &parent_dir);
    let (tx, rx) = channel();
    let mut watcher: RecommendedWatcher = Watcher::new(tx, std::time::Duration::from_millis(200))?;
    watcher.watch(parent_dir, RecursiveMode::Recursive)?;

    let mut server_binary = std::env::current_exe()?;
    server_binary.set_file_name("clay-server");

    let claypot_file_name = format!("{}pot", model_path.to_str().unwrap());

    fn should_restart(path: &Path) -> bool {
        !matches!(path.extension().and_then(|e| e.to_str()), Some("claypot"))
    }

    // this method attempts to builds a claypot from the model and spawn a clay-server from it

    // - if the attempt succeeds, we will return a handle to the process in an Ok(Some(...))
    // - if the return value is an Err, this means that we have encountered an unrecoverable error, and so the
    //   watcher should exit.
    // - if the return value is an Ok(None), this mean that we have encountered some error, but it is not necessarily
    //   unrecoverable (the watcher should not exit)
    let build_and_start_server: &dyn Fn() -> Result<Option<Child>> = &|| {
        let result = build(&absolute_path, None, false).and_then(|_| {
            if let Err(e) = prestart_callback() {
                println!("Error: {}", e);
            }

            let mut command = std::process::Command::new(&server_binary);
            command.args(vec![&claypot_file_name]);
            if let Some(port) = server_port {
                command.env("CLAY_SERVER_PORT", port.to_string());
            }
            command
                .spawn()
                .context("Failed to start clay-server")
                .map_err(|e| BuildError::UnrecoverableError(anyhow!(e)))
        });

        match result {
            // server successfully started
            Ok(child) => Ok(Some(child)),

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

    let mut server = build_and_start_server()?;

    // watcher loop
    loop {
        if crate::SIGINT.load(Ordering::SeqCst) {
            break;
        }

        // block loop for 500ms
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(event) => match &event {
                DebouncedEvent::Create(path) | DebouncedEvent::Write(path) => {
                    if should_restart(path) {
                        println!("Change detected, rebuilding and restarting...");

                        if let Some(server) = server.as_mut() {
                            if server.kill().is_err() {
                                println!("Unable to kill server");
                            }
                        }

                        server = build_and_start_server()?;
                    }
                }
                _ => {}
            },
            Err(e) => match e {
                RecvTimeoutError::Timeout => {}
                RecvTimeoutError::Disconnected => {
                    println!("watch error: {:?}", e);
                    break;
                }
            },
        }

        if let Some(server) = server.as_mut() {
            if let Ok(Some(_)) = server.try_wait() {
                // server died for some reason
                break;
            }
        }
    }

    if let Some(mut server) = server {
        let _ = server.kill();
    }

    Ok(())
}
