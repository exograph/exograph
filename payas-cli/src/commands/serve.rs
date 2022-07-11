use anyhow::Result;
use std::{path::PathBuf, time::SystemTime};

use crate::util::watcher;

use super::command::Command;

/// Run local claytip server
pub struct ServeCommand {
    pub model: PathBuf,
    pub port: Option<u32>,
}

impl Command for ServeCommand {
    fn run(&self, _system_start_time: Option<SystemTime>) -> Result<()> {
        watcher::start_watcher(&self.model, self.port, || Ok(()))
    }
}
