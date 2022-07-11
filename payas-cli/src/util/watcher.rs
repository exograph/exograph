use std::{
    path::Path,
    sync::{
        atomic::Ordering,
        mpsc::{channel, RecvTimeoutError},
    },
    time::Duration,
};

use anyhow::{Context, Result};
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
        .expect("Couldn't get model as canonical path");
    let parent_dir = absolute_path
        .parent()
        .expect("Couldn't get parent directory");
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

    let start_server = || {
        build(&absolute_path, None, false).and_then(|_| {
            prestart_callback()?;

            let mut command = std::process::Command::new(&server_binary);
            command.args(vec![&claypot_file_name]);
            if let Some(port) = server_port {
                command.env("CLAY_SERVER_PORT", port.to_string());
            }
            command.spawn().context("Failed to start clay-server")
        })
    };

    let mut server = start_server();

    loop {
        if crate::SIGINT.load(Ordering::SeqCst) {
            break;
        }

        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(event) => match &event {
                DebouncedEvent::Create(path) | DebouncedEvent::Write(path) => {
                    if should_restart(path) {
                        println!("Change detected, rebuilding and restarting...");

                        if let Ok(mut server) = server {
                            if server.kill().is_err() {
                                println!("Unable to kill server");
                            }
                        }

                        server = start_server();
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
        server.kill()?;
    }
    Ok(())
}
