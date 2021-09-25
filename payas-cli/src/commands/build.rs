use anyhow::Result;
use payas_server::watcher;

use std::path::Path;
use std::time::Duration;
use std::{fs::File, io::BufWriter};
use std::{path::PathBuf, time::SystemTime};

use bincode::serialize_into;
use payas_parser::{builder, parser};

use super::Command;

const FILE_WATCHER_DELAY: Duration = Duration::from_millis(10);

/// Build claytip server binary
pub struct BuildCommand {
    pub model: PathBuf,
    pub watch: bool,
}

impl Command for BuildCommand {
    fn run(&self, system_start_time: Option<SystemTime>) -> Result<()> {
        let build_fn = move |restart| {
            let system_start_time = if restart {
                Some(SystemTime::now())
            } else {
                system_start_time
            };

            build(&self.model, system_start_time, restart)
        };

        if !self.watch {
            build_fn(false)
        } else {
            watcher::with_watch(&self.model, FILE_WATCHER_DELAY, build_fn, |_: &mut ()| ())
        }
    }
}

fn build(model: &Path, system_start_time: Option<SystemTime>, _restart: bool) -> Result<()> {
    let (ast_system, codemap) = parser::parse_file(&model);
    let system = builder::build(ast_system, codemap)?;

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
