//! Top level subcommands

use std::path::PathBuf;

pub mod model;
pub mod schema;

pub trait Command {
    fn run(&self) -> Result<(), String>;
}

/// Build claytip server binary
#[derive(Debug)]
pub struct BuildCommand {
    pub model: PathBuf,
}

impl Command for BuildCommand {
    fn run(&self) -> Result<(), String> {
        println!("{:#?}", self);
        Ok(())
    }
}

/// Perform a database migration for a claytip model
#[derive(Debug)]
pub struct MigrateCommand {
    pub model: PathBuf,
    pub database: String,
}

impl Command for MigrateCommand {
    fn run(&self) -> Result<(), String> {
        println!("{:#?}", self);
        Ok(())
    }
}

/// Claytip model utilities
#[derive(Debug)]
pub struct ServeCommand {
    pub model: PathBuf,
}

impl Command for ServeCommand {
    fn run(&self) -> Result<(), String> {
        println!("{:#?}", self);
        Ok(())
    }
}

/// Perform integration tests
#[derive(Debug)]
pub struct TestCommand {
    pub dir: PathBuf,
}

impl Command for TestCommand {
    fn run(&self) -> Result<(), String> {
        println!("{:#?}", self);
        Ok(())
    }
}

/// Verify that a schema is compatible with a claytip model
#[derive(Debug)]
pub struct VerifyCommand {
    pub model: PathBuf,
    pub database: String,
}

impl Command for VerifyCommand {
    fn run(&self) -> Result<(), String> {
        println!("{:#?}", self);
        Ok(())
    }
}

/// Run local claytip server with a temporary database
#[derive(Debug)]
pub struct YoloCommand {
    pub model: PathBuf,
}

impl Command for YoloCommand {
    fn run(&self) -> Result<(), String> {
        println!("{:#?}", self);
        Ok(())
    }
}
