use anyhow::Result;
use clap::Command;
use std::{io::Write, path::PathBuf};

use exo_sql::schema::spec::SchemaSpec;

use crate::{
    commands::command::{get, get_required, model_file_arg, output_arg, CommandDefinition},
    util::open_file_for_output,
};

use super::{migration_helper::migration_statements, util};

pub(super) struct CreateCommandDefinition {}

impl CommandDefinition for CreateCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("create")
            .about("Create a database schema from a Exograph model")
            .arg(model_file_arg())
            .arg(output_arg())
    }

    /// Create a database schema from a exograph model
    fn execute(&self, matches: &clap::ArgMatches) -> Result<()> {
        let model: PathBuf = get_required(matches, "model")?;
        let output: Option<PathBuf> = get(matches, "output");

        let postgres_subsystem = util::create_postgres_system(&model)?;

        let mut buffer: Box<dyn Write> = open_file_for_output(output.as_deref())?;

        // Creating the schema from the model is the same as migrating from an empty database.
        for (statement, _) in migration_statements(
            &SchemaSpec::default(),
            &SchemaSpec::from_model(postgres_subsystem.tables.into_iter().collect()),
        ) {
            writeln!(buffer, "{statement}\n")?;
        }

        Ok(())
    }
}
