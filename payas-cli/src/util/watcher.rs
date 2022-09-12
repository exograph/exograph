use std::{
    path::Path,
    sync::{
        atomic::Ordering,
        mpsc::{channel, RecvTimeoutError},
    },
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use notify::{DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};

use crate::commands::build::build;

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

    let build_and_start_server = || {
        build(&absolute_path, None, false).and_then(|_| {
            if let Err(e) = prestart_callback() {
                println!("Error: {}", e);
            }

            let mut command = std::process::Command::new(&server_binary);
            command.args(vec![&claypot_file_name]);
            if let Some(port) = server_port {
                command.env("CLAY_SERVER_PORT", port.to_string());
            }
            command.spawn().context("Failed to start clay-server")
        })
    };

    let mut server = build_and_start_server();

    loop {
        if crate::SIGINT.load(Ordering::SeqCst) {
            break;
        }

        if let Ok(child) = server.as_mut() {
            if let Ok(Some(_)) = child.try_wait() {
                // server has exited for some reason, break out of loop so we can exit
                break;
            }
        }

        // block loop for 500ms
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(event) => match &event {
                DebouncedEvent::Create(path) | DebouncedEvent::Write(path) => {
                    if should_restart(path) {
                        println!("Change detected, rebuilding and restarting...");

                        if let Ok(server) = server.as_mut() {
                            if server.kill().is_err() {
                                println!("Unable to kill server");
                            }
                        }

                        server = build_and_start_server();
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
    }

    if let Ok(mut server) = server {
        let _ = server.kill();
    }
    Ok(())
}
