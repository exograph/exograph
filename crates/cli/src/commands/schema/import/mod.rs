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
use exo_sql::DatabaseClient;
use exo_sql::schema::database_spec::DatabaseSpec;
use exo_sql::schema::issue::WithIssues;
use exo_sql::schema::spec::{MigrationScope, MigrationScopeMatches};

use std::io::Write;
use std::path::PathBuf;

use crate::commands::command::{
    CommandDefinition, database_arg, database_value, get, migration_scope_arg,
    migration_scope_value, mutation_access_arg, mutation_access_value, output_arg,
    query_access_arg, query_access_value, yes_arg, yes_value,
};
use crate::commands::util::compute_migration_scope;
use crate::config::Config;
use crate::util::open_file_for_output;

use super::util::open_database;

mod context;
mod processor;

mod column_processor;
mod database_processor;
mod enum_processor;
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

        let mut writer = open_file_for_output(output.as_deref(), yes)?;
        let db_client = open_database(database_url.as_deref()).await?;
        let db_client = db_client.get_client().await?;

        create_exo_model(
            &mut writer,
            &db_client,
            query_access,
            mutation_access,
            generate_fragments,
            scope,
        )
        .await?;

        if let Some(output) = &output {
            eprintln!("\nExograph model written to `{}`", output.display());
        }

        Ok(())
    }
}

pub(crate) async fn create_exo_model(
    mut writer: impl Write + Send,
    db_client: &DatabaseClient,
    query_access: bool,
    mutation_access: bool,
    generate_fragments: bool,
    scope: Option<String>,
) -> Result<()> {
    let schema = import_schema(db_client, &compute_migration_scope(scope)).await?;

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

async fn import_schema(
    client: &DatabaseClient,
    scope: &MigrationScope,
) -> Result<WithIssues<DatabaseSpec>> {
    let scope_matches = match scope {
        MigrationScope::Specified(scope) => scope,
        MigrationScope::FromNewSpec => &MigrationScopeMatches::all_schemas(),
    };

    let database = DatabaseSpec::from_live_database(client, scope_matches).await?;
    Ok(database)
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use std::io::BufWriter;

    use common::test_support::{assert_file_content, read_relative_file};
    use exo_sql::{
        Database, schema::migration::Migration, testing::test_support::with_init_script,
    };
    use postgres_core_model::subsystem::PostgresCoreSubsystem;

    use crate::commands::build::build_system_with_static_builders;

    use super::*;

    #[tokio::test]
    async fn test_import_schema() {
        common::test_support::run_tests(
            env!("CARGO_MANIFEST_DIR"),
            "_EXO_IMPORT_TEST_FILTER",
            "src/commands/schema/import/test-data",
            |folder, test_path| async move { single_test(folder, test_path).await },
        )
        .await
        .unwrap();
    }

    async fn single_test(
        test_name: String,
        test_path: PathBuf,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let schema = read_relative_file(&test_path, "schema.sql").unwrap();

        with_init_script(&schema, |client| async move {
            let mut writer = BufWriter::new(Vec::new());
            create_exo_model(&mut writer, &client, true, false, false, None)
                .await
                .unwrap();

            let output = String::from_utf8(writer.into_inner().unwrap()).unwrap();
            assert_file_content(&test_path, "index.expected.exo", &output, &test_name)?;

            let expected_model_file = test_path.join("index.expected.exo");

            let serialized_system =
                build_system_with_static_builders(&expected_model_file, None, None)
                    .await
                    .unwrap();

            let postgres_subsystem = serialized_system
                .subsystems
                .into_iter()
                .find(|subsystem| subsystem.id == "postgres");

            use core_plugin_shared::system_serializer::SystemSerializer;
            let database = match postgres_subsystem {
                Some(subsystem) => {
                    PostgresCoreSubsystem::deserialize(subsystem.core.0)
                        .unwrap()
                        .database
                }
                None => Database::default(),
            };

            Migration::verify(&client, &database, &MigrationScope::all_schemas())
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
        })
        .await?;

        Ok(())
    }
}
