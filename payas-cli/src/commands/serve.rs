use anyhow::Result;
use std::{path::PathBuf, time::SystemTime};

use crate::util::watcher;

use super::{command::Command, schema::verify::VerifyCommand};

/// Run local claytip server
pub struct ServeCommand {
    pub model: PathBuf,
    pub port: Option<u32>,
}

impl Command for ServeCommand {
    fn run(&self, _system_start_time: Option<SystemTime>) -> Result<()> {
        watcher::start_watcher(&self.model, self.port, || {
            println!("Verifying new model...");
            let verify_command = VerifyCommand {
                model: self.model.clone(),
                database: None,
            };

            verify_command.run(None)
        })
    }
}
