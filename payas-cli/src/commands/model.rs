//! Subcommands under the `model` subcommand

use std::path::PathBuf;

use super::Command;

/// Create a claytip model file based on a database schema
#[derive(Debug)]
pub struct ImportCommand {
    pub database: String,
    pub output: PathBuf,
}

impl Command for ImportCommand {
    fn run(&self) -> Result<(), String> {
        println!("{:#?}", self);
        Ok(())
    }
}
