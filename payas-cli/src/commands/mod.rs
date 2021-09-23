//! Top level subcommands

use anyhow::Result;
use bincode::serialize_into;
use payas_parser::{builder, parser};
use std::{fs::File, io::BufWriter, path::PathBuf, time::SystemTime};

pub mod model;
pub mod schema;

pub trait Command {
    fn run(&self, system_start_time: Option<SystemTime>) -> Result<()>;
}

/// Build claytip server binary
pub struct BuildCommand {
    pub model: PathBuf,
}

impl Command for BuildCommand {
    fn run(&self, system_start_time: Option<SystemTime>) -> Result<()> {
        let (ast_system, codemap) = parser::parse_file(&self.model);
        let system = builder::build(ast_system, codemap)?;

        let claypot_file_name = format!("{}pot", &self.model.to_str().unwrap());

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
