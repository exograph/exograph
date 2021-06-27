//! Subcommands under the `model` subcommand

use anyhow::Result;
use std::path::PathBuf;

use super::Command;

/// Create a claytip model file based on a database schema
pub struct ImportCommand {
    pub database: String,
    pub output: PathBuf,
}

impl Command for ImportCommand {
    fn run(&self) -> Result<()> {
        todo!("Implmement model import command");
    }
}
