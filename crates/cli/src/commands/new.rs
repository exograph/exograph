use std::fs::{create_dir_all, File};
use std::io::Write;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::{ArgMatches, Command};

use super::command::{get_required, new_project_arg, CommandDefinition};

static POSTGRES_TEMPLATE: &[u8] = include_bytes!("templates/postgres.exo");
static GITIGNORE_TEMPLATE: &[u8] = include_bytes!("templates/gitignore");

pub struct NewCommandDefinition {}

impl CommandDefinition for NewCommandDefinition {
    fn command(&self) -> Command {
        Command::new("new")
            .about("Create a new Exograph project")
            .arg(new_project_arg())
    }

    fn execute(&self, matches: &ArgMatches) -> Result<()> {
        let path: PathBuf = get_required(matches, "path")?;

        if path.exists() {
            return Err(anyhow!(
                "The path '{}' already exists. Please choose a different name.",
                path.display()
            ));
        }

        create_dir_all(&path)?;

        let mut model_file = File::create(path.join("index.exo"))?;
        model_file.write_all(POSTGRES_TEMPLATE)?;

        let mut gitignore_file = File::create(path.join(".gitignore"))?;
        gitignore_file.write_all(GITIGNORE_TEMPLATE)?;

        Ok(())
    }
}
