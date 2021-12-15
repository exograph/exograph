use std::{path::PathBuf, time::SystemTime};

use super::command::Command;
use anyhow::Result;

/// Perform integration tests
pub struct TestCommand {
    pub dir: PathBuf,
}

impl Command for TestCommand {
    fn run(&self, _system_start_time: Option<SystemTime>) -> Result<()> {
        payas_test::run(&self.dir)
    }
}
