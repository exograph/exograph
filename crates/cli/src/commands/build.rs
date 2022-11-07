use anyhow::{anyhow, Result};
use builder::error::ParserError;

use std::error::Error;
use std::ffi::OsStr;
use std::fmt::Display;
use std::io::Write;
use std::path::Path;
use std::{fs::File, io::BufWriter};
use std::{path::PathBuf, time::SystemTime};

use super::command::Command;

/// Build claytip server binary
pub struct BuildCommand {
    pub model: PathBuf,
}

impl Command for BuildCommand {
    fn run(&self, system_start_time: Option<SystemTime>) -> Result<()> {
        Ok(build(&self.model, system_start_time, true)?)
    }
}

#[derive(Debug)]
pub enum BuildError {
    ParserError(ParserError),
    UnrecoverableError(anyhow::Error),
}

impl Error for BuildError {}

impl Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            BuildError::ParserError(e) => writeln!(f, "Parser error: {}", e),
            BuildError::UnrecoverableError(e) => writeln!(f, "{}", e),
        }
    }
}

/// Build claypot file
///
/// # Arguments
/// * `model` - claytip model path
/// * `system_start_time` - system start time. If specified, it will print a message indicated the time it took to build the model
/// * `print_message` - if true, it will print a message indicating the time it took to build the model. We need this
///                        to avoid printing the message when building the model through `clay serve`, where we don't want to print the message
///                        upon detecting changes
pub(crate) fn build(
    model: &Path,
    system_start_time: Option<SystemTime>,
    print_message: bool,
) -> Result<(), BuildError> {
    let serialized_system = builder::build_system(model).map_err(BuildError::ParserError)?;

    let claypot_file_name = {
        if let Some("clay") = model.extension().and_then(OsStr::to_str) {
            let mut filename = model.to_path_buf();
            filename.set_extension("claypot");
            filename
        } else {
            return Err(BuildError::UnrecoverableError(anyhow!(
                "{} is not a clay file",
                model.display()
            )));
        }
    };

    let mut out_file = BufWriter::new(File::create(&claypot_file_name).unwrap());
    out_file.write_all(&serialized_system).unwrap();

    if print_message {
        match system_start_time {
            Some(system_start_time) => {
                let elapsed = system_start_time
                    .elapsed()
                    .map_err(|e| BuildError::UnrecoverableError(anyhow!(e)))?
                    .as_millis();
                println!(
                    "Claypot file '{}' created in {} milliseconds",
                    claypot_file_name.display(),
                    elapsed
                );
            }
            None => {
                println!("Claypot file {} created", claypot_file_name.display());
            }
        }

        println!(
            "You can start the server with using the 'clay-server {}' command",
            claypot_file_name.display()
        );
    }

    Ok(())
}
