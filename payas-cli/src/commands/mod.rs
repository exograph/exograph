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
    pub watch: bool,
}

impl Command for ServeCommand {
    fn run(&self) -> Result<()> {
        payas_server::main(self.model.clone(), self.watch)
    }
}

/// Perform integration tests
pub struct TestCommand {
    pub dir: PathBuf,
}

impl Command for TestCommand {
    fn run(&self) -> Result<()> {
        payas_test::run(&self.dir)
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
