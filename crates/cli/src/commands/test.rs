use std::{path::PathBuf, time::SystemTime};

use super::command::Command;
use anyhow::Result;

/// Perform integration tests
pub struct TestCommand {
    pub dir: PathBuf,
    pub pattern: Option<String>, // glob pattern indicating tests to be executed
}

impl Command for TestCommand {
    fn run(&self, _system_start_time: Option<SystemTime>) -> Result<()> {
        testing::run(&self.dir, &self.pattern)
    }
}
