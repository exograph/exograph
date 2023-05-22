// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use anyhow::Result;
use async_trait::async_trait;
use clap::Command;
use std::{io::Write, path::PathBuf};

use exo_sql::schema::database_spec::DatabaseSpec;

use crate::{
    commands::command::{
        default_model_file, ensure_exo_project_dir, get, output_arg, CommandDefinition,
    },
    util::open_file_for_output,
};

use super::{migration::Migration, util};

pub(super) struct CreateCommandDefinition {}

#[async_trait]
impl CommandDefinition for CreateCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("create")
            .about("Create a database schema from a Exograph model")
            .arg(output_arg())
    }

    /// Create a database schema from a exograph model
    async fn execute(&self, matches: &clap::ArgMatches) -> Result<()> {
        ensure_exo_project_dir(&PathBuf::from("."))?;

        let model: PathBuf = default_model_file();
        let output: Option<PathBuf> = get(matches, "output");

        let postgres_subsystem = util::create_postgres_system(model).await?;

        let mut buffer: Box<dyn Write> = open_file_for_output(output.as_deref())?;

        // Creating the schema from the model is the same as migrating from an empty database.
        let migrations = Migration::from_schemas(
            &DatabaseSpec::new(vec![]),
            &DatabaseSpec::from_database(postgres_subsystem.database),
        );
        migrations.write(&mut buffer, true)?;

        Ok(())
    }
}
