// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{io, path::PathBuf};

use anyhow::anyhow;
use exo_sql::{database_error::DatabaseError, DatabaseClientManager};
use postgres_model::migration::Migration;

use crate::{
    commands::command::{
        database_arg, default_model_file, ensure_exo_project_dir, get, output_arg,
        CommandDefinition,
    },
    util::open_file_for_output,
};

use super::util;
use anyhow::Result;
use async_trait::async_trait;
use clap::{Arg, Command};

pub(super) struct MigrateCommandDefinition {}

#[async_trait]
impl CommandDefinition for MigrateCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("migrate")
        .about("Produces a SQL migration script for a Exograph model and the specified database")
        .arg(database_arg())
        .arg(output_arg())
        .arg(
            Arg::new("apply-to-database")
                .help("Apply non-destructive migration to the database specified by the --database flag or the environment variable EXO_POSTGRES_URL")
                .long("apply-to-database")
                .required(false)
                .num_args(0)
        )
        .arg(
            Arg::new("allow-destructive-changes")
                .help("By default, destructive changes in the model file are commented out. If specified, this option will uncomment such changes")
                .long("allow-destructive-changes")
                .required(false)
                .num_args(0),
        )
    }

    /// Perform a database migration for a exograph model
    async fn execute(&self, matches: &clap::ArgMatches) -> Result<()> {
        ensure_exo_project_dir(&PathBuf::from("."))?;

        let model: PathBuf = default_model_file();
        let database: Option<String> = get(matches, "database");
        let output: Option<PathBuf> = get(matches, "output");
        let apply_to_database: bool = matches.get_flag("apply-to-database");
        let allow_destructive_changes: bool = matches.get_flag("allow-destructive-changes");

        if output.is_some() && apply_to_database {
            return Err(anyhow!(
                "Cannot specify both --output and --apply-to-database"
            ));
        }

        let postgres_subsystem = util::create_postgres_system(&model, None).await?;
        let db_client = open_database(database.as_deref()).await?;
        let migrations = Migration::from_db_and_model(&db_client, &postgres_subsystem).await?;

        if apply_to_database {
            if migrations.has_destructive_changes() {
                Err(anyhow!("Migration contains destructive changes"))
            } else {
                migrations.apply(&db_client, false).await?;
                Ok(())
            }
        } else {
            let mut buffer: Box<dyn io::Write> = open_file_for_output(output.as_deref())?;
            migrations.write(&mut buffer, allow_destructive_changes)?;
            Ok(())
        }
    }
}

pub async fn open_database(database: Option<&str>) -> Result<DatabaseClientManager, DatabaseError> {
    if let Some(database) = database {
        Ok(DatabaseClientManager::from_url(database, true, None).await?)
    } else {
        Ok(util::database_manager_from_env().await?)
    }
}
