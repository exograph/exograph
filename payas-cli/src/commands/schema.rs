//! Subcommands under the `schema` subcommand

use anyhow::Result;
use payas_model::spec::FromModel;
use std::{path::PathBuf, time::SystemTime};

use payas_sql::schema::spec::SchemaSpec;

use crate::commands::migrate::migration_statements;

use super::command::Command;

/// Create a database schema from a claytip model
pub struct CreateCommand {
    pub model: PathBuf,
}

impl Command for CreateCommand {
    fn run(&self, _system_start_time: Option<SystemTime>) -> Result<()> {
        let system = payas_parser::build_system(&self.model)?;

        for (statement, _) in
            migration_statements(SchemaSpec::default(), SchemaSpec::from_model(system.tables))
        {
            println!("{}\n", statement);
        }

        Ok(())
    }
}

/// Verify that a schema is compatible with a claytip model
pub struct VerifyCommand {
    pub model: PathBuf,
    pub database: String,
}

impl Command for VerifyCommand {
    fn run(&self, _system_start_time: Option<SystemTime>) -> Result<()> {
        todo!("Implement verify command");
    }
}
