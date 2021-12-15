use std::{path::PathBuf, time::SystemTime};

use super::command::Command;
use anyhow::Result;

/// Run local claytip server with a temporary database
pub struct YoloCommand {
    pub model: PathBuf,
}

impl Command for YoloCommand {
    fn run(&self, _system_start_time: Option<SystemTime>) -> Result<()> {
        todo!("Implmement yolo command");
    }
}
