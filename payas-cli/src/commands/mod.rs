//! Top level subcommands

use anyhow::Result;
use std::{path::PathBuf, time::SystemTime};

pub mod build;
pub mod import;
pub mod schema;

pub trait Command {
    fn run(&self, system_start_time: Option<SystemTime>) -> Result<()>;
}

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

/// Perform integration tests
pub struct TestCommand {
    pub dir: PathBuf,
}

impl Command for TestCommand {
    fn run(&self, _system_start_time: Option<SystemTime>) -> Result<()> {
        payas_test::run(&self.dir)
    }
}

/// Run local claytip server with a temporary database
pub struct YoloCommand {
    pub model: PathBuf,
}

impl Command for YoloCommand {
    fn run(&self, _system_start_time: Option<SystemTime>) -> Result<()> {
        todo!("Implmement yolo command");
    }
}
