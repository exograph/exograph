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
use colored::Colorize;
use exo_sql::schema::database_spec::DatabaseSpec;
use exo_sql::schema::migration::{
    migrate_interactively, Migration, MigrationError, MigrationInteraction, TableAction,
};
use exo_sql::SchemaObjectName;
use exo_sql::{database_error::DatabaseError, DatabaseClientManager};

use crate::commands::command::{
    database_value, migration_scope_arg, migration_scope_value, yes_arg, yes_value,
};
use crate::config::Config;
use crate::{
    commands::{
        command::{database_arg, default_model_file, get, output_arg, CommandDefinition},
        util::{compute_migration_scope, use_ir_arg},
    },
    util::open_file_for_output,
};

use super::util::{self, open_database};
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
        .arg(migration_scope_arg())
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
        .arg(use_ir_arg())
        .arg(yes_arg())
    }

    /// Perform a database migration for a exograph model
    async fn execute(&self, matches: &clap::ArgMatches, _config: &Config) -> Result<()> {
        let model: PathBuf = default_model_file();
        let database_url = database_value(matches);
        let output: Option<PathBuf> = get(matches, "output");
        let apply_to_database: bool = matches.get_flag("apply-to-database");
        let allow_destructive_changes: bool = matches.get_flag("allow-destructive-changes");
        let use_ir: bool = matches.get_flag("use-ir");
        let scope: Option<String> = migration_scope_value(matches);
        let yes: bool = yes_value(matches);

        if output.is_some() && apply_to_database {
            return Err(anyhow!(
                "Cannot specify both --output and --apply-to-database"
            ));
        }

        let db_client = open_database(database_url.as_deref()).await?;
        let mut db_client = db_client.get_client().await?;

        let database = util::extract_postgres_database(&model, None, use_ir).await?;
        let migrations =
            Migration::from_db_and_model(&db_client, &database, &compute_migration_scope(scope))
                .await?;

        if apply_to_database {
            migrations
                .apply(&mut db_client, allow_destructive_changes)
                .await?;
            Ok(())
        } else {
            let mut buffer: Box<dyn io::Write> = open_file_for_output(output.as_deref(), yes)?;
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

pub async fn migrate_interactively_from_db_and_model(
    model: &PathBuf,
    use_ir: bool,
    db_client: &DatabaseClientManager,
) -> Result<Migration> {
    let database = util::extract_postgres_database(&model, None, use_ir).await?;
    let new_db_spec = DatabaseSpec::from_database(&database);

    let old_db_spec =
        Migration::extract_schema_from_db(db_client, &new_db_spec, &compute_migration_scope(None))
            .await?;

    Ok(migrate_interactively(
        old_db_spec.value,
        new_db_spec,
        &compute_migration_scope(None),
        &UserMigrationInteraction,
    )
    .await?)
}

const MANUAL_HANDLE: &str = "Let me handle this manually";
const RENAME_HANDLE: &str = "Rename it";
const DELETE_HANDLE: &str = "Delete it";

struct UserMigrationInteraction;

impl MigrationInteraction for UserMigrationInteraction {
    fn handle_table_delete(
        &self,
        deleted_table: &SchemaObjectName,
        create_tables: Vec<&SchemaObjectName>,
    ) -> Result<TableAction, MigrationError> {
        let options = if create_tables.is_empty() {
            vec![MANUAL_HANDLE, DELETE_HANDLE]
        } else {
            vec![MANUAL_HANDLE, RENAME_HANDLE, DELETE_HANDLE]
        };

        println!(
            "\nThe database has the {} table that doesn't exist in the new schema.",
            deleted_table.fully_qualified_name_with_sep(".").red()
        );
        let ans = inquire::Select::new("How would you like to handle this table?", options)
            .prompt()
            .map_err(|e| MigrationError::Generic(e.to_string()))?;

        match ans {
            MANUAL_HANDLE => Ok(TableAction::Manual(deleted_table.clone())),
            RENAME_HANDLE => {
                let create_table_displays = create_tables
                    .iter()
                    .map(|table| TableDisplay(table))
                    .collect::<Vec<_>>();
                let new_name =
                    inquire::Select::new("What should the new name be?", create_table_displays)
                        .prompt()
                        .map_err(|e| MigrationError::Generic(e.to_string()))?;
                Ok(TableAction::Rename(
                    deleted_table.clone(),
                    new_name.0.clone(),
                ))
            }
            DELETE_HANDLE => Ok(TableAction::Delete(deleted_table.clone())),
            _ => unreachable!(),
        }
    }
}

struct TableDisplay<'a>(&'a SchemaObjectName);

impl<'a> std::fmt::Display for TableDisplay<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.fully_qualified_name_with_sep("."))
    }
}
