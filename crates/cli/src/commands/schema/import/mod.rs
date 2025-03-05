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
use clap::{Arg, Command};
use exo_sql::schema::database_spec::DatabaseSpec;
use exo_sql::schema::issue::WithIssues;
use exo_sql::schema::spec::{MigrationScope, MigrationScopeMatches};

use std::path::PathBuf;

use crate::commands::command::{
    database_arg, get, migration_scope_arg, output_arg, CommandDefinition,
};
use crate::commands::util::compute_migration_scope;
use crate::config::Config;
use crate::util::open_file_for_output;

use super::util::open_database;

mod context;
mod processor;

mod column_processor;
mod database_processor;
mod table_processor;

use context::ImportContext;
use processor::ModelProcessor;
pub(super) struct ImportCommandDefinition {}

#[async_trait]
impl CommandDefinition for ImportCommandDefinition {
    fn command(&self) -> clap::Command {
        Command::new("import")
            .about("Create exograph model file based on a database schema")
            .arg(database_arg())
            .arg(output_arg())
            .arg(migration_scope_arg())
            .arg(
                Arg::new("access")
                    .help("Access expression to apply to all tables (default: false)")
                    .long("access")
                    .required(false)
                    .value_parser(clap::value_parser!(bool))
                    .num_args(1),
            )
            .arg(
                Arg::new("generate-fragments")
                    .help("Generate fragments for tables")
                    .long("generate-fragments")
                    .required(false)
                    .num_args(0),
            )
    }

    /// Create a exograph model file based on a database schema
    async fn execute(&self, matches: &clap::ArgMatches, _config: &Config) -> Result<()> {
        let output: Option<PathBuf> = get(matches, "output");
        let database_url: Option<String> = get(matches, "database");
        let access: bool = get(matches, "access").unwrap_or(false);
        let generate_fragments: bool = matches.get_flag("generate-fragments");
        let scope: Option<String> = get(matches, "scope");

        let mut writer = open_file_for_output(output.as_deref())?;

        let schema = import_schema(database_url, &compute_migration_scope(scope)).await?;

        let mut context = ImportContext::new(&schema.value, access, generate_fragments);

        for table in &schema.value.tables {
            context.add_table(&table.name);
        }

        schema.value.process(&context, &mut writer)?;

        for issue in &schema.issues {
            eprintln!("{issue}");
        }

        if let Some(output) = &output {
            eprintln!("\nExograph model written to `{}`", output.display());
        }

        Ok(())
    }
}

async fn import_schema(
    database_url: Option<String>,
    scope: &MigrationScope,
) -> Result<WithIssues<DatabaseSpec>> {
    let db_client = open_database(database_url.as_deref()).await?;
    let client = db_client.get_client().await?;

    let scope_matches = match scope {
        MigrationScope::Specified(scope) => scope,
        MigrationScope::FromNewSpec => &MigrationScopeMatches::all_schemas(),
    };

    let database = DatabaseSpec::from_live_database(&client, scope_matches).await?;
    Ok(database)
}
