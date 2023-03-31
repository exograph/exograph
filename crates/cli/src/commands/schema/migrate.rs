use std::{io, path::PathBuf, time::SystemTime};

use crate::{
    commands::command::Command,
    util::{open_database, open_file_for_output},
};

use super::{migration_helper::migration_statements, util};
use anyhow::Result;
use exo_sql::schema::spec::SchemaSpec;

/// Perform a database migration for a exograph model
pub struct MigrateCommand {
    pub model: PathBuf,
    pub database: Option<String>,
    pub output: Option<PathBuf>,
    pub allow_destructive_changes: bool,
}

impl Command for MigrateCommand {
    fn run(&self, _system_start_time: Option<SystemTime>) -> Result<()> {
        let mut buffer: Box<dyn io::Write> = open_file_for_output(self.output.as_deref())?;

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .unwrap();

        rt.block_on(async {
            let database = open_database(self.database.as_deref())?;
            let client = database.get_client().await?;

            let old_schema = SchemaSpec::from_db(&client).await?;

            for issue in &old_schema.issues {
                eprintln!("{issue}");
            }

            let new_postgres_subsystem = util::create_postgres_system(&self.model)?;

            let new_schema =
                SchemaSpec::from_model(new_postgres_subsystem.tables.into_iter().collect());

            let statements = migration_statements(&old_schema.value, &new_schema);

            for (statement, is_destructive) in statements {
                if is_destructive && !self.allow_destructive_changes {
                    write!(buffer, "-- ")?;
                }
                writeln!(buffer, "{statement}")?;
            }

            Ok(())
        })
    }
}
