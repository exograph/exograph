// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{io, path::PathBuf};

use crate::{
    commands::command::{
        database_arg, get, get_required, model_file_arg, output_arg, CommandDefinition,
    },
    util::{open_database, open_file_for_output},
};

use super::{migration_helper::migration_statements, util};
use anyhow::Result;
use clap::{Arg, Command};
use exo_sql::schema::spec::SchemaSpec;

pub(super) struct MigrateCommandDefinition {}

impl CommandDefinition for MigrateCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("migrate")
        .about("Produces a SQL migration script for a Exograph model and the specified database")
        .arg(model_file_arg())
        .arg(database_arg())
        .arg(output_arg())
        .arg(
            Arg::new("allow-destructive-changes")
                .help("By default, destructive changes in the model file are commented out. If specified, this option will uncomment such changes.")
                .long("allow-destructive-changes")
                .required(false)
                .num_args(0),
        )
    }

    /// Perform a database migration for a exograph model
    fn execute(&self, matches: &clap::ArgMatches) -> Result<()> {
        let model: PathBuf = get_required(matches, "model")?;
        let database: Option<String> = get(matches, "database");
        let output: Option<PathBuf> = get(matches, "output");
        let allow_destructive_changes: bool = matches.get_flag("allow-destructive-changes");

        let mut buffer: Box<dyn io::Write> = open_file_for_output(output.as_deref())?;

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .unwrap();

        rt.block_on(async {
            let database = open_database(database.as_deref())?;
            let client = database.get_client().await?;

            let old_schema = SchemaSpec::from_db(&client).await?;

            for issue in &old_schema.issues {
                eprintln!("{issue}");
            }

            let new_postgres_subsystem = util::create_postgres_system(&model)?;

            let new_schema =
                SchemaSpec::from_model(new_postgres_subsystem.tables.into_iter().collect());

            let statements = migration_statements(&old_schema.value, &new_schema);

            for (statement, is_destructive) in statements {
                if is_destructive && !allow_destructive_changes {
                    write!(buffer, "-- ")?;
                }
                writeln!(buffer, "{statement}")?;
            }

            Ok(())
        })
    }
}
