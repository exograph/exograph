use std::{path::PathBuf, time::SystemTime};

use super::command::Command;
use anyhow::{Context, Result};

/// Run local claytip server
pub struct ServeCommand {
    pub model: PathBuf,
    pub watch: bool,
}

impl Command for ServeCommand {
    fn run(&self, system_start_time: Option<SystemTime>) -> Result<()> {
        super::build::build(&self.model, system_start_time)?;

        let mut server_binary = std::env::current_exe()?;
        server_binary.set_file_name("clay-server");

        let claypot_file_name = format!("{}pot", &self.model.to_str().unwrap());
        //payas_server::start_dev_mode(self.model.clone(), self.watch, system_start_time)
        let mut server = std::process::Command::new(server_binary)
            .args(vec![claypot_file_name])
            .spawn()
            .context("Failed to start clay-server")?;
        server
            .wait()
            .context("server not running when wait called")?;
        Ok(())
    }
}
