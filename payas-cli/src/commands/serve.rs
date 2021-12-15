use std::{path::PathBuf, time::SystemTime};

use super::command::Command;
use anyhow::Result;

/// Run local claytip server
pub struct ServeCommand {
    pub model: PathBuf,
    pub watch: bool,
}

impl Command for ServeCommand {
    fn run(&self, system_start_time: Option<SystemTime>) -> Result<()> {
        payas_server::start_dev_mode(self.model.clone(), self.watch, system_start_time)
    }
}
