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
use colored::Colorize;
use exo_env::Environment;
use exo_sql::schema::database_spec::DatabaseSpec;
use exo_sql::schema::issue::WithIssues;
use exo_sql::schema::spec::{MigrationScope, MigrationScopeMatches};
use exo_sql::{DatabaseClient, SchemaObjectName, TransactionMode};

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

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
mod traits;

mod column_processor;
pub mod database_processor;
pub mod enum_processor;
pub mod table_processor;

use context::ImportContext;
use traits::{ImportWriter, ModelImporter};
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
    async fn execute(
        &self,
        matches: &clap::ArgMatches,
        _config: &Config,
        env: Arc<dyn Environment>,
    ) -> Result<()> {
        let output: Option<PathBuf> = get(matches, "output");
        let database_url = database_value(matches);
        let query_access: bool = query_access_value(matches);
        let mutation_access: bool = mutation_access_value(matches);
        let generate_fragments: bool = matches.get_flag("generate-fragments");
        let scope: Option<String> = migration_scope_value(matches);
        let yes: bool = yes_value(matches);

        let mut writer = open_file_for_output(output.as_deref(), yes)?;
        let db_client = open_database(
            database_url.as_deref(),
            TransactionMode::ReadOnly,
            env.as_ref(),
        )
        .await?;
        let db_client = db_client.get_client().await?;

        let table_names = create_exo_model(
            &mut writer,
            &db_client,
            query_access,
            mutation_access,
            generate_fragments,
            scope,
        )
        .await?;

        if let Some(output) = &output {
            println!("Imported tables:");
            print_imported_tables(table_names, 80);
            println!("\nExograph model written to `{}`", output.display());
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
) -> Result<Vec<SchemaObjectName>> {
    let schema = import_schema(db_client, &compute_migration_scope(scope)).await?;

    let mut context = ImportContext::new(
        &schema.value,
        query_access,
        mutation_access,
        generate_fragments,
    );

    let table_names = schema
        .value
        .tables
        .iter()
        .map(|table| table.name.clone())
        .collect::<Vec<_>>();

    for table in &schema.value.tables {
        context.add_table(&table.name);
    }

    let database_import = schema.value.to_import(&(), &context)?;
    database_import.write_to(&mut writer)?;

    for issue in &schema.issues {
        eprintln!("{issue}");
    }

    Ok(table_names)
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

pub(crate) fn print_imported_tables(table_names: Vec<SchemaObjectName>, max_width: usize) {
    // Group tables by schema
    let mut tables_by_schema: std::collections::HashMap<String, Vec<&SchemaObjectName>> =
        std::collections::HashMap::new();

    for table_name in &table_names {
        let schema = table_name.schema.as_deref().unwrap_or("public");
        tables_by_schema
            .entry(schema.to_string())
            .or_default()
            .push(table_name);
    }

    // Sort schemas and tables within each schema
    let mut sorted_schemas: Vec<_> = tables_by_schema.keys().collect();
    sorted_schemas.sort();

    for schema in sorted_schemas {
        print!("    {}:", schema.bold().purple());
        let mut tables = tables_by_schema[schema].clone();
        tables.sort_by(|a, b| a.name.cmp(&b.name));

        let available_width = max_width - 8; // schema is indented 4 spaces, and table starts with 4 spaces
        let mut current_line_width = 0;

        for (i, table) in tables.iter().enumerate() {
            let needs_comma = i > 0;
            let text_width = table.name.len() + if needs_comma { 2 } else { 0 }; // +2 for comma and space

            if current_line_width == 0 || current_line_width + text_width <= available_width {
                if current_line_width == 0 {
                    print!("\n        {}", table.name.bold().cyan());
                    current_line_width = table.name.len();
                } else {
                    print!(", {}", table.name.bold().cyan());
                    current_line_width += text_width;
                }
            } else {
                print!("\n        {}", table.name.bold().cyan());
                current_line_width = table.name.len();
            }
        }

        println!();
    }
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
