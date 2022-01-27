use anyhow::Result;

use std::path::Path;
use std::{fs::File, io::BufWriter};
use std::{path::PathBuf, time::SystemTime};

use bincode::serialize_into;

use super::command::Command;

/// Build claytip server binary
pub struct BuildCommand {
    pub model: PathBuf,
}

impl Command for BuildCommand {
    fn run(&self, system_start_time: Option<SystemTime>) -> Result<()> {
        build(&self.model, system_start_time)
    }
}

pub(crate) fn build(model: &Path, system_start_time: Option<SystemTime>) -> Result<()> {
    let system = payas_parser::build_system(&model)?;

    let claypot_file_name = format!("{}pot", &model.to_str().unwrap());

    let mut out_file = BufWriter::new(File::create(&claypot_file_name).unwrap());
    serialize_into(&mut out_file, &system).unwrap();

    match system_start_time {
        Some(system_start_time) => {
            let elapsed = system_start_time.elapsed()?.as_millis();
            println!(
                "Claypot file '{}' created in {} milliseconds",
                claypot_file_name, elapsed
            );
        }
        None => {
            println!("Claypot file {} created", claypot_file_name);
        }
    }

    println!(
        "You can start the server with using the 'clay-server {}' command",
        claypot_file_name
    );

    Ok(())
}
