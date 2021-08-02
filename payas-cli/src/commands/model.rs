//! Subcommands under the `model` subcommand

use anyhow::Result;
use payas_model::sql::database::Database;
use payas_sql::spec::SchemaSpec;
use std::{fs::File, io::Write, path::PathBuf};

use super::Command;

/// Create a claytip model file based on a database schema
pub struct ImportCommand {
    pub output: PathBuf,
}

impl Command for ImportCommand {
    fn run(&self) -> Result<()> {
        let database = Database::from_env()?; // TODO: error handling here
        database.create_client()?;

        let schema = SchemaSpec::from_db(&database)?;
        File::create(&self.output)?.write_all(schema.spec.to_model().as_bytes())?;
        schema.issues.iter().for_each(|i| println!("{}", i));
        println!("\nClaytip model written to `{}`", self.output.display());
        Ok(())
    }
}
