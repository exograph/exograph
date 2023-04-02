use std::fs::{create_dir_all, File};
use std::io::Write;
use std::path::PathBuf;
use std::time::SystemTime;

use anyhow::{anyhow, Result};

use super::command::Command;

/// Create a new exograph project
pub struct NewCommand {
    pub path: PathBuf,
}

static POSTGRES_TEMPLATE: &[u8] = include_bytes!("templates/postgres.exo");
static GITIGNORE_TEMPLATE: &[u8] = include_bytes!("templates/gitignore");

impl Command for NewCommand {
    fn run(&self, _system_start_time: Option<SystemTime>) -> Result<()> {
        if self.path.exists() {
            return Err(anyhow!(
                "The path '{}' already exists. Please choose a different name.",
                self.path.display()
            ));
        }

        create_dir_all(&self.path)?;

        let mut model_file = File::create(self.path.join("index.exo"))?;
        model_file.write_all(POSTGRES_TEMPLATE)?;

        let mut gitignore_file = File::create(self.path.join(".gitignore"))?;
        gitignore_file.write_all(GITIGNORE_TEMPLATE)?;

        Ok(())
    }
}
