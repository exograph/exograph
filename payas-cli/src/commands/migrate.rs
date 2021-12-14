use std::{path::PathBuf, time::SystemTime};

use super::command::Command;
use anyhow::Result;

/// Perform a database migration for a claytip model
pub struct MigrateCommand {
    pub model: PathBuf,
    pub database: String,
}

impl Command for MigrateCommand {
    fn run(&self, _system_start_time: Option<SystemTime>) -> Result<()> {
        todo!("Implmement migrate command");
    }
}
