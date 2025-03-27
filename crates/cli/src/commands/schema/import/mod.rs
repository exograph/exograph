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

use std::path::{Path, PathBuf};

use crate::commands::command::{
    database_arg, database_value, get, migration_scope_arg, migration_scope_value,
    mutation_access_arg, mutation_access_value, output_arg, query_access_arg, query_access_value,
    yes_arg, yes_value, CommandDefinition,
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
            .arg(query_access_arg())
            .arg(mutation_access_arg())
            .arg(
                Arg::new("generate-fragments")
                    .help("Generate fragments for tables")
                    .long("generate-fragments")
                    .required(false)
                    .num_args(0),
            )
            .arg(yes_arg())
    }

    /// Create a exograph model file based on a database schema
    async fn execute(&self, matches: &clap::ArgMatches, _config: &Config) -> Result<()> {
        let output: Option<PathBuf> = get(matches, "output");
        let database_url = database_value(matches);
        let query_access: bool = query_access_value(matches);
        let mutation_access: bool = mutation_access_value(matches);
        let generate_fragments: bool = matches.get_flag("generate-fragments");
        let scope: Option<String> = migration_scope_value(matches);
        let yes: bool = yes_value(matches);

        create_model_file(
            output.as_deref(),
            database_url,
            query_access,
            mutation_access,
            generate_fragments,
            scope,
            yes,
        )
        .await?;

        if let Some(output) = &output {
            eprintln!("\nExograph model written to `{}`", output.display());
        }

        Ok(())
    }
}

pub(crate) async fn create_model_file(
    output: Option<&Path>,
    database_url: Option<String>,
    query_access: bool,
    mutation_access: bool,
    generate_fragments: bool,
    scope: Option<String>,
    yes: bool,
) -> Result<()> {
    let mut writer = open_file_for_output(output, yes)?;

    let schema = import_schema(database_url, &compute_migration_scope(scope)).await?;

    let mut context = ImportContext::new(
        &schema.value,
        query_access,
        mutation_access,
        generate_fragments,
    );

    for table in &schema.value.tables {
        context.add_table(&table.name);
    }

    schema.value.process(&(), &context, &mut writer)?;

    for issue in &schema.issues {
        eprintln!("{issue}");
    }

    Ok(())
}

pub(crate) async fn import_schema(
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
