use anyhow::{anyhow, Result};
use std::{
    io::{stdin, stdout, Write},
    path::PathBuf,
    time::SystemTime,
};

use crate::{
    commands::schema::verify::{verify, VerificationErrors},
    util::watcher,
};

use super::command::Command;

/// Run local claytip server
pub struct ServeCommand {
    pub model: PathBuf,
    pub port: Option<u32>,
}

impl Command for ServeCommand {
    fn run(&self, _system_start_time: Option<SystemTime>) -> Result<()> {
        watcher::start_watcher(&self.model, self.port, || {
            println!("Verifying new model...");

            loop {
                let verification_result = verify(&self.model, None);

                match verification_result {
                    Err(e @ VerificationErrors::ModelNotCompatible(_)) => {
                        println!("The schema of the current database is not compatible with the current model for the following reasons:");
                        println!("{}", e);
                        println!("Select an option:");
                        print!("[c]ontinue without fixing, (p)ause and fix manually: ");
                        stdout().flush()?;

                        let mut input: String = String::new();
                        let result = std::io::stdin()
                            .read_line(&mut input)
                            .map(|_| input.trim())?;

                        match result {
                            "p" => {
                                println!("Paused. Press enter to re-verify.");

                                let mut line = String::new();
                                stdin().read_line(&mut line)?;
                            }
                            _ => {
                                println!("Continuing...");
                                break Ok(());
                            }
                        }
                    }
                    _ => {
                        break verification_result
                            .map_err(|e| anyhow!("Error during verification: {}", e))
                    }
                }
            }
        })
    }
}
