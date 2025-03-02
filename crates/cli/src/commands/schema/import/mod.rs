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
use exo_sql::schema::database_spec::DatabaseSpec;
use exo_sql::schema::issue::WithIssues;
use exo_sql::schema::spec::{MigrationScope, MigrationScopeMatches};

use std::path::PathBuf;

use crate::commands::command::{database_arg, get, output_arg, CommandDefinition};
use crate::commands::util::migration_scope_from_env;
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
    }

    /// Create a exograph model file based on a database schema
    async fn execute(&self, matches: &clap::ArgMatches, _config: &Config) -> Result<()> {
        let output: Option<PathBuf> = get(matches, "output");
        let database_url: Option<String> = get(matches, "database");

        let mut writer = open_file_for_output(output.as_deref())?;

        let mut context = ImportContext::new();
        let mut schema = import_schema(database_url, &migration_scope_from_env()).await?;
        schema.value.process(&mut context, &mut writer)?;

        context.add_issues(&mut schema.issues);

        for issue in &context.issues {
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
