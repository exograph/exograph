//! Subcommands under the `model` subcommand

use anyhow::Result;
use payas_model::spec::ToModel;
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

        let mut issues = Vec::new();

        let mut schema = SchemaSpec::from_db(&database)?;
        let mut model = schema.value.to_model();

        issues.append(&mut schema.issues);
        issues.append(&mut model.issues);

        File::create(&self.output)?.write_all(schema.value.to_model().value.as_bytes())?;
        for issue in &issues {
            println!("{}", issue);
        }

        println!("\nClaytip model written to `{}`", self.output.display());
        Ok(())
    }
}
