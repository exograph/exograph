use std::time::SystemTime;

use anyhow::Result;

pub trait Command {
    fn run(&self, system_start_time: Option<SystemTime>) -> Result<()>;
}
