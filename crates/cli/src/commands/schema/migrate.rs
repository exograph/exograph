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
        database_arg, default_model_file, ensure_exo_project_dir, get, output_arg,
        CommandDefinition,
    },
    util::open_file_for_output,
};

use super::migration::Migration;
use anyhow::Result;
use clap::{Arg, Command};

pub(super) struct MigrateCommandDefinition {}

impl CommandDefinition for MigrateCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("migrate")
        .about("Produces a SQL migration script for a Exograph model and the specified database")
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
        ensure_exo_project_dir(&PathBuf::from("."))?;

        let model: PathBuf = default_model_file();
        let database: Option<String> = get(matches, "database");
        let output: Option<PathBuf> = get(matches, "output");
        let allow_destructive_changes: bool = matches.get_flag("allow-destructive-changes");

        let mut buffer: Box<dyn io::Write> = open_file_for_output(output.as_deref())?;

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .unwrap();

        rt.block_on(async {
            let migrations = Migration::from_db_and_model(database.as_deref(), &model).await?;
            migrations.write(&mut buffer, allow_destructive_changes)?;

            Ok(())
        })
    }
}
