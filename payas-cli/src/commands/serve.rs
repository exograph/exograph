use anyhow::{Context, Result};
use notify::{DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::mpsc::channel;
use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

use super::command::Command;

/// Run local claytip server
pub struct ServeCommand {
    pub model: PathBuf,
    pub watch: bool,
}

impl Command for ServeCommand {
    fn run(&self, system_start_time: Option<SystemTime>) -> Result<()> {
        super::build::build(&self.model, system_start_time)?;

        let absolute_path = self
            .model
            .as_path()
            .canonicalize()
            .expect("Couldn't get model as canonical path");
        let parent_dir = absolute_path
            .parent()
            .expect("Couldn't get parent directory");
        println!("Watching: {:?}", &absolute_path);
        let (tx, rx) = channel();
        let mut watcher: RecommendedWatcher = Watcher::new(tx, std::time::Duration::from_secs(2))?;
        watcher.watch(parent_dir, RecursiveMode::Recursive)?;

        let mut server_binary = std::env::current_exe()?;
        server_binary.set_file_name("clay-server");

        let claypot_file_name = format!("{}pot", &self.model.to_str().unwrap());

        let start_server = || {
            std::process::Command::new(&server_binary)
                .args(vec![&claypot_file_name])
                .spawn()
                .context("Failed to start clay-server")
        };

        fn should_restart(path: &Path) -> bool {
            match path.extension().and_then(|e| e.to_str()) {
                Some("claypot") => false,
                Some(_) => true,
                None => false,
            }
        }

        let mut server = start_server()?;

        loop {
            match rx.recv() {
                Ok(event) => match &event {
                    DebouncedEvent::Create(path) | DebouncedEvent::Write(path) => {
                        if should_restart(path) {
                            println!("Change detected, rebuilding and restarting...");

                            if server.kill().is_err() {
                                println!("Unable to kill server");
                            }
                            super::build::build(&self.model, None)?;
                            server = start_server()?;
                        }
                    }
                    _ => {}
                },
                Err(e) => {
                    println!("watch error: {:?}", e);
                    break;
                }
            }
        }
        server.kill()?;
        Ok(())
    }
}
