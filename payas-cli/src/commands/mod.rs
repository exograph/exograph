//! Top level subcommands

use anyhow::Result;
use std::path::PathBuf;

pub mod model;
pub mod schema;

pub trait Command {
    fn run(&self) -> Result<()>;
}

/// Build claytip server binary
pub struct BuildCommand {
    pub model: PathBuf,
}

impl Command for BuildCommand {
    fn run(&self) -> Result<()> {
        todo!("Implmement build command");
    }
}

/// Perform a database migration for a claytip model
pub struct MigrateCommand {
    pub model: PathBuf,
    pub database: String,
}

impl Command for MigrateCommand {
    fn run(&self) -> Result<()> {
        todo!("Implmement migrate command");
    }
}

/// Claytip model utilities
pub struct ServeCommand {
    pub model: PathBuf,
}

impl Command for ServeCommand {
    fn run(&self) -> Result<()> {
        todo!("Implmement serve command");
    }
}

/// Perform integration tests
pub struct TestCommand {
    pub dir: PathBuf,
}

impl Command for TestCommand {
    fn run(&self) -> Result<()> {
        todo!("Implmement test command");
    }
}

/// Verify that a schema is compatible with a claytip model
pub struct VerifyCommand {
    pub model: PathBuf,
    pub database: String,
}

impl Command for VerifyCommand {
    fn run(&self) -> Result<()> {
        todo!("Implmement verify command");
    }
}

/// Run local claytip server with a temporary database
pub struct YoloCommand {
    pub model: PathBuf,
}

impl Command for YoloCommand {
    fn run(&self) -> Result<()> {
        todo!("Implmement yolo command");
    }
}
