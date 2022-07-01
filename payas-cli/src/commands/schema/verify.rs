//! Subcommands under the `schema` subcommand

use anyhow::Result;
use std::{path::PathBuf, time::SystemTime};

use crate::commands::command::Command;

/// Verify that a schema is compatible with a claytip model
pub struct VerifyCommand {
    pub model: PathBuf,
    pub database: String,
}

impl Command for VerifyCommand {
    fn run(&self, _system_start_time: Option<SystemTime>) -> Result<()> {
        todo!("Implement verify command");
    }
}
