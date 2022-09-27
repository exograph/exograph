use anyhow::Result;
use std::{io::Write, path::PathBuf, time::SystemTime};

use payas_sql::schema::spec::SchemaSpec;

use crate::{commands::command::Command, util::open_file_for_output};

use super::migration_helper::migration_statements;

/// Create a database schema from a claytip model
pub struct CreateCommand {
    pub model: PathBuf,
    pub output: Option<PathBuf>,
}

impl Command for CreateCommand {
    fn run(&self, _system_start_time: Option<SystemTime>) -> Result<()> {
        let system = payas_parser::build_system(&self.model)?;

        let mut buffer: Box<dyn Write> = open_file_for_output(self.output.as_deref())?;

        // Creating the schema from the model is the same as migrating from an empty database.
        for (statement, _) in migration_statements(
            &SchemaSpec::default(),
            &SchemaSpec::from_model(system.database_subsystem.tables.into_iter().collect()),
        ) {
            writeln!(buffer, "{}\n", statement)?;
        }

        Ok(())
    }
}
