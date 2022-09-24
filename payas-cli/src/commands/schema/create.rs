use anyhow::Result;
use std::{path::PathBuf, time::SystemTime};

use payas_sql::schema::spec::SchemaSpec;

use crate::commands::command::Command;

use super::migration_helper::migration_statements;

/// Create a database schema from a claytip model
pub struct CreateCommand {
    pub model: PathBuf,
}

impl Command for CreateCommand {
    fn run(&self, _system_start_time: Option<SystemTime>) -> Result<()> {
        let system = payas_parser::build_system(&self.model)?;

        // Creating the schema from the model is the same as migrating from an empty database.
        for (statement, _) in migration_statements(
            &SchemaSpec::default(),
            &SchemaSpec::from_model(system.database_subsystem.tables.into_iter().collect()),
        ) {
            println!("{}\n", statement);
        }

        Ok(())
    }
}
