// Copyright Exograph, Inc. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file at the root of this repository.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::fmt::Display;

use exo_sql::{
    database_error::DatabaseError,
    schema::{
        database_spec::DatabaseSpec,
        issue::WithIssues,
        op::SchemaOp,
        spec::{diff, MigrationScope, MigrationScopeMatches},
    },
    Database, DatabaseClientManager,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Migration {
    pub statements: Vec<MigrationStatement>,
}

#[derive(Debug, Serialize)]
pub struct MigrationStatement {
    pub statement: String,
    pub is_destructive: bool,
}

pub enum VerificationErrors {
    PostgresError(DatabaseError),
    ModelNotCompatible(Vec<String>),
}

impl From<DatabaseError> for VerificationErrors {
    fn from(e: DatabaseError) -> Self {
        VerificationErrors::PostgresError(e)
    }
}

impl Display for VerificationErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerificationErrors::PostgresError(e) => write!(f, "Postgres error: {e}"),
            VerificationErrors::ModelNotCompatible(e) => {
                for error in e.iter() {
                    writeln!(f, "- {error}")?
                }

                Ok(())
            }
        }
    }
}

impl Migration {
    pub fn from_schemas(
        old_schema_spec: &DatabaseSpec,
        new_schema_spec: &DatabaseSpec,
        scope: &MigrationScope,
    ) -> Self {
        let mut pre_statements = vec![];
        let mut statements = vec![];
        let mut post_statements = vec![];

        let diffs = diff(old_schema_spec, new_schema_spec, scope);

        for diff in diffs.iter() {
            let is_destructive = match diff {
                SchemaOp::DeleteSchema { .. }
                | SchemaOp::DeleteTable { .. }
                | SchemaOp::DeleteColumn { .. }
                | SchemaOp::RemoveExtension { .. } => true,

                // Explicitly matching the other cases here to ensure that we have thought about each case
                SchemaOp::CreateSchema { .. }
                | SchemaOp::CreateTable { .. }
                | SchemaOp::CreateColumn { .. }
                | SchemaOp::CreateIndex { .. }
                | SchemaOp::DeleteIndex { .. } // Creating and deleting index is not considered destructive (they affect performance but not data loss)
                | SchemaOp::CreateExtension { .. }
                | SchemaOp::CreateUniqueConstraint { .. }
                | SchemaOp::RemoveUniqueConstraint { .. }
                | SchemaOp::CreateForeignKeyReference { .. }
                | SchemaOp::DeleteForeignKeyReference { .. }
                | SchemaOp::SetColumnDefaultValue { .. }
                | SchemaOp::UnsetColumnDefaultValue { .. }
                | SchemaOp::SetNotNull { .. }
                | SchemaOp::UnsetNotNull { .. }
                | SchemaOp::CreateFunction { .. }
                | SchemaOp::DeleteFunction { .. }
                | SchemaOp::CreateOrReplaceFunction { .. }
                | SchemaOp::CreateTrigger { .. }
                | SchemaOp::DeleteTrigger { .. } => false,
            };

            let statement = diff.to_sql();

            for constraint in statement.pre_statements.into_iter() {
                if !constraint.trim().is_empty() {
                    pre_statements.push(MigrationStatement::new(constraint, is_destructive));
                }
            }

            if !statement.statement.trim().is_empty() {
                statements.push(MigrationStatement::new(statement.statement, is_destructive));
            }

            for constraint in statement.post_statements.into_iter() {
                if !constraint.trim().is_empty() {
                    post_statements.push(MigrationStatement::new(constraint, is_destructive));
                }
            }
        }

        pre_statements.extend(statements);
        pre_statements.extend(post_statements);

        Migration {
            statements: pre_statements,
        }
    }

    pub async fn from_db_and_model(
        client: &DatabaseClientManager,
        database: &Database,
        scope: &MigrationScope,
    ) -> Result<Self, DatabaseError> {
        let database_spec = DatabaseSpec::from_database(database);

        let scope_matches = match scope {
            MigrationScope::Specified(scope) => scope,
            MigrationScope::FromNewSpec => {
                &MigrationScopeMatches::from_specs_schemas(&[&database_spec])
            }
        };

        let old_schema = extract_db_schema(client, scope_matches).await?;

        for issue in &old_schema.issues {
            eprintln!("{issue}");
        }

        Ok(Migration::from_schemas(
            &old_schema.value,
            &database_spec,
            scope,
        ))
    }

    pub fn has_destructive_changes(&self) -> bool {
        self.statements
            .iter()
            .any(|statement| statement.is_destructive)
    }

    pub async fn verify(
        client: &DatabaseClientManager,
        database: &Database,
        scope: &MigrationScope,
    ) -> Result<(), VerificationErrors> {
        let new_schema = DatabaseSpec::from_database(database);

        let scope_matches = match scope {
            MigrationScope::Specified(scope) => scope,
            MigrationScope::FromNewSpec => {
                &MigrationScopeMatches::from_specs_schemas(&[&new_schema])
            }
        };

        let old_schema = extract_db_schema(client, scope_matches).await?;

        for issue in &old_schema.issues {
            eprintln!("{issue}");
        }

        let diff = diff(&old_schema.value, &new_schema, scope);

        let errors: Vec<_> = diff.iter().flat_map(|op| op.error_string()).collect();

        if !errors.is_empty() {
            Err(VerificationErrors::ModelNotCompatible(errors))
        } else {
            Ok(())
        }
    }

    pub async fn apply(
        &self,
        database: &DatabaseClientManager,
        allow_destructive_changes: bool,
    ) -> Result<(), anyhow::Error> {
        let mut client = database.get_client().await?;
        let transaction = client.transaction().await?;
        for MigrationStatement {
            statement,
            is_destructive,
        } in self.statements.iter()
        {
            if !is_destructive || allow_destructive_changes {
                transaction.execute(statement, &[]).await?;
            } else {
                return Err(anyhow::anyhow!(
                    "Destructive change detected: {}",
                    statement
                ));
            }
        }
        Ok(transaction.commit().await?)
    }

    pub fn write(
        &self,
        writer: &mut dyn std::io::Write,
        allow_destructive_changes: bool,
    ) -> std::io::Result<()> {
        for MigrationStatement {
            statement,
            is_destructive,
        } in self.statements.iter()
        {
            if *is_destructive && !allow_destructive_changes {
                write!(writer, "-- ")?;
            }
            writeln!(writer, "{statement}\n")?;
        }
        Ok(())
    }
}

impl MigrationStatement {
    pub fn new(statement: String, is_destructive: bool) -> Self {
        Self {
            statement,
            is_destructive,
        }
    }
}

async fn extract_db_schema(
    database: &DatabaseClientManager,
    scope: &MigrationScopeMatches,
) -> Result<WithIssues<DatabaseSpec>, DatabaseError> {
    let client = database.get_client().await?;

    DatabaseSpec::from_live_database(&client, scope).await
}

pub async fn wipe_database(database: &DatabaseClientManager) -> Result<(), DatabaseError> {
    let client = database.get_client().await?;

    // wiping is equivalent to migrating to an empty database and deals with non-public schemas correctly
    let current_database_spec =
        &DatabaseSpec::from_live_database(&client, &MigrationScopeMatches::all_schemas())
            .await
            .map_err(|e| DatabaseError::BoxedError(Box::new(e)))?
            .value;

    let migrations = Migration::from_schemas(
        current_database_spec,
        &DatabaseSpec::new(vec![], vec![]),
        &MigrationScope::all_schemas(),
    );
    migrations
        .apply(database, true)
        .await
        .map_err(|e| DatabaseError::BoxedError(e.into()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::subsystem::PostgresCoreSubsystem;
    use core_plugin_shared::{
        error::ModelSerializationError, serializable_system::SerializableSystem,
    };
    use sqlparser::dialect::PostgreSqlDialect;
    use sqlparser::parser::Parser;

    use colored::Colorize;
    use wildmatch::WildMatch;

    use super::*;

    #[cfg_attr(not(target_family = "wasm"), tokio::test)]
    #[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
    async fn all_tests() {
        let filter = std::env::var("_EXO_DEV_MIGRATION_TEST_FILTER").unwrap_or("*".to_string());
        let wildcard = WildMatch::new(&filter);

        let test_configs_dir = relative_path("", "");
        let test_configs = std::fs::read_dir(test_configs_dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().unwrap().is_dir())
            .filter(|entry| wildcard.matches(entry.file_name().to_str().unwrap()));

        let mut failed = false;

        for test_config in test_configs {
            let test_config_name = test_config.file_name();
            let test_config_name = test_config_name.to_str().unwrap();
            if let Err(e) = single_test(test_config_name).await {
                println!("{}: {}", test_config_name, "failed".red());
                println!("{}", e);
                failed = true;
            }
        }

        if failed {
            panic!("{}", "Some tests failed".red());
        }
    }

    async fn single_test(folder: &str) -> Result<(), String> {
        println!("Testing {}", folder);
        let old_exo = read_relative_file(folder, "old/src/index.exo")
            .map_err(|e| format!("Failed to read old exo: {}", e))?;
        let new_exo = read_relative_file(folder, "new/src/index.exo")
            .map_err(|e| format!("Failed to read new exo: {}", e))?;

        let old_system = compute_spec(&old_exo).await;
        let new_system = compute_spec(&new_exo).await;

        let scope_dirs = std::fs::read_dir(relative_path(folder, ""))
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().unwrap().is_dir())
            .filter(|entry| entry.file_name().to_str().unwrap().starts_with("scope-"));

        let mut failed = false;

        for scope_dir in scope_dirs {
            let scope_dir_name = scope_dir.file_name().to_str().unwrap().to_owned();
            let scope_spec_name = scope_dir_name.replace("scope-", "");
            let scope = if scope_spec_name == "all-schemas" {
                Ok(MigrationScope::all_schemas())
            } else if scope_spec_name == "new-spec" {
                Ok(MigrationScope::FromNewSpec)
            } else {
                Err(format!("Unknown scope: {}", scope_dir_name))
            }?;

            let scope_folder = format!("{}/{}", folder, scope_dir_name);

            println!("\tscope {}:", scope_spec_name);

            if let Err(e) = assert_for_scope(&old_system, &new_system, &scope_folder, &scope).await
            {
                println!("{}: {}", scope_folder, e);
                failed = true;
            }
        }

        if failed {
            Err(format!("{}: Some tests failed", folder))
        } else {
            Ok(())
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    enum TestKind {
        OldCreation,
        NewCreation,
        IdempotentSelfMigration,
        Up,
        Down,
    }

    impl TestKind {
        fn kind_str(&self) -> &str {
            match self {
                TestKind::OldCreation => "old",
                TestKind::NewCreation => "new",
                TestKind::IdempotentSelfMigration => "idempotent",
                TestKind::Up => "up",
                TestKind::Down => "down",
            }
        }
    }

    async fn assert_for_scope(
        old_system: &DatabaseSpec,
        new_system: &DatabaseSpec,
        folder: &str,
        scope: &MigrationScope,
    ) -> Result<(), String> {
        let old_expected_sql = read_relative_file(folder, "old.sql").unwrap_or_default();
        let new_expected_sql = read_relative_file(folder, "new.sql").unwrap_or_default();
        let up_expected_sql = read_relative_file(folder, "up.sql").unwrap_or_default();
        let down_expected_sql = read_relative_file(folder, "down.sql").unwrap_or_default();

        let mut failed = false;

        if let Err(e) = assert_creation_and_self_migration(
            old_system,
            &old_expected_sql,
            scope,
            folder,
            TestKind::OldCreation,
        ) {
            println!("Old creation failed: {}", e);
            failed = true;
        } else {
            println!("\t\told-creation: {}", "pass".green());
        }

        if let Err(e) = assert_creation_and_self_migration(
            new_system,
            &new_expected_sql,
            scope,
            folder,
            TestKind::NewCreation,
        ) {
            println!("New creation failed: {}", e);
            failed = true;
        } else {
            println!("\t\tnew-creation: {}", "pass".green());
        }

        if let Err(e) = assert_migration(
            old_system,
            new_system,
            &up_expected_sql,
            scope,
            folder,
            TestKind::Up,
        ) {
            println!("Up failed: {}", e);
            failed = true;
        } else {
            println!("\t\tup: {}", "pass".green());
        }

        if let Err(e) = assert_migration(
            new_system,
            old_system,
            &down_expected_sql,
            scope,
            folder,
            TestKind::Down,
        ) {
            println!("Down failed: {}", e);
            failed = true;
        } else {
            println!("\t\tdown: {}", "pass".green());
        }

        if failed {
            Err(format!("{}: Tests for scope {:?} failed", folder, scope))
        } else {
            Ok(())
        }
    }

    fn assert_creation_and_self_migration(
        system: &DatabaseSpec,
        expected: &str,
        migration_scope: &MigrationScope,
        folder: &str,
        kind: TestKind,
    ) -> Result<(), String> {
        let creation =
            Migration::from_schemas(&DatabaseSpec::new(vec![], vec![]), system, migration_scope);
        assert_sql_eq(creation, expected, folder, kind)?;

        let self_migration = Migration::from_schemas(system, system, migration_scope);
        assert_sql_eq(
            self_migration,
            "",
            folder,
            TestKind::IdempotentSelfMigration,
        )?;

        Ok(())
    }

    fn assert_migration(
        old_system: &DatabaseSpec,
        new_system: &DatabaseSpec,
        expected: &str,
        migration_scope: &MigrationScope,
        folder: &str,
        kind: TestKind,
    ) -> Result<(), String> {
        let migration = Migration::from_schemas(old_system, new_system, migration_scope);
        assert_sql_eq(migration, expected, folder, kind)
    }

    fn assert_sql_eq(
        actual: Migration,
        expected: &str,
        folder: &str,
        kind: TestKind,
    ) -> Result<(), String> {
        {
            // Check if strings match. This lets us avoid parsing the SQL (which in some cases doesn't work with syntax such as pgvector indexes)
            // TODO: Contribute to sqlparser to support pgvector and other cases where parsing fails
            let mut buffer = std::io::Cursor::new(vec![]);
            actual.write(&mut buffer, false).unwrap();
            let actual_sql = String::from_utf8(buffer.into_inner()).unwrap();

            if actual_sql.lines().count() == expected.lines().count()
                && actual_sql
                    .lines()
                    .zip(expected.lines())
                    .all(|(a, e)| a.trim() == e.trim())
            {
                return Ok(());
            }
        }

        let (actual_sql, actual_destructive_indices) = {
            let actual_sql_destructive_indices = actual
                .statements
                .iter()
                .enumerate()
                .filter_map(|(index, stmt)| {
                    if stmt.is_destructive {
                        Some(index)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            let actual_destructive_migration = Migration {
                statements: actual
                    .statements
                    .iter()
                    .map(|stmt| MigrationStatement {
                        statement: stmt.statement.clone(),
                        is_destructive: false,
                    })
                    .collect::<Vec<_>>(),
            };

            let mut buffer = std::io::Cursor::new(vec![]);
            actual_destructive_migration
                .write(&mut buffer, true)
                .unwrap();
            let actual_sql_str = String::from_utf8(buffer.into_inner()).unwrap();
            (actual_sql_str, actual_sql_destructive_indices)
        };

        let (expected_sql, expected_destructive_indices) = {
            let expected_sql = expected.split(";\n").map(|s| s.trim()).collect::<Vec<_>>();
            let expected_sql_destructive_indices = expected_sql
                .iter()
                .enumerate()
                .filter_map(|(index, s)| {
                    if s.starts_with("-- ") {
                        Some(index)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            let expected_sql_destructive = expected_sql
                .into_iter()
                .map(|stmt| stmt.strip_prefix("-- ").unwrap_or(stmt).to_string())
                .collect::<Vec<_>>()
                .join(";\n");
            (expected_sql_destructive, expected_sql_destructive_indices)
        };

        let message = format!("{} {}", folder, kind.kind_str());

        if let Err(e) = assert_sql_str_eq(&actual_sql, &expected_sql, &message) {
            dump_migration(&actual, folder, kind)
                .map_err(|e| format!("Failed to dump migration: {}", e))?;
            return Err(e);
        }

        if actual_destructive_indices != expected_destructive_indices {
            return Err(format!(
                "{}: Destructive indices are different.\n  Expected: {:?}\n  Actual:   {:?}",
                message, expected_destructive_indices, actual_destructive_indices,
            ));
        }

        Ok(())
    }

    fn dump_migration(
        migration: &Migration,
        folder: &str,
        kind: TestKind,
    ) -> Result<(), std::io::Error> {
        let kind_str = kind.kind_str();

        let file_name = relative_path(folder, &format!("{}.actual.sql", kind_str));
        let mut file = std::fs::File::create(file_name)?;
        migration.write(&mut file, false)?;
        Ok(())
    }

    fn assert_sql_str_eq(actual: &str, expected: &str, message: &str) -> Result<(), String> {
        // Line-ending insensitive comparison (for Windows compatibility)
        if actual.lines().count() == expected.lines().count()
            && (actual.lines().zip(expected.lines())).all(|(a, e)| a.trim() == e.trim())
        {
            return Ok(());
        }

        let dialect = PostgreSqlDialect {};
        let actual_sql = Parser::parse_sql(&dialect, actual)
            .map_err(|e| format!("Failed to parse actual SQL: {}", e))?;
        let expected_sql = Parser::parse_sql(&dialect, expected)
            .map_err(|e| format!("Failed to parse expected SQL: {}", e))?;

        if actual_sql != expected_sql {
            if actual_sql.len() != expected_sql.len() {
                return Err(format!(
                    "{}: Actual SQL length {} is different from expected SQL length {}",
                    message,
                    actual_sql.len(),
                    expected_sql.len(),
                ));
            }

            let mut messages = vec![];

            actual_sql
                .iter()
                .zip(expected_sql.iter())
                .enumerate()
                .for_each(|(i, (a, e))| {
                    if a != e {
                        messages.push(format!(
                            "{}: Statement at index {} is different.\n  Expected: {}\n  Actual:   {}",
                            message,
                            i,
                            e,
                            a
                        ));
                    }
                });

            if !messages.is_empty() {
                return Err(messages.join("\n"));
            }
        }

        Ok(())
    }

    fn relative_path(folder: &str, path: &str) -> PathBuf {
        let base_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/migration-test-data");

        if folder.is_empty() {
            return base_path;
        }

        let folder_path = base_path.join(folder);

        if path.is_empty() {
            return folder_path;
        }

        folder_path.join(path)
    }

    fn read_relative_file(folder: &str, path: &str) -> Result<String, std::io::Error> {
        std::fs::read_to_string(relative_path(folder, path))
    }

    async fn create_postgres_system_from_str(
        model_str: &str,
        file_name: String,
    ) -> Result<PostgresCoreSubsystem, ModelSerializationError> {
        let system = builder::build_system_from_str(
            model_str,
            file_name,
            vec![Box::new(
                postgres_builder::PostgresSubsystemBuilder::default(),
            )],
        )
        .await
        .unwrap();

        deserialize_postgres_subsystem(system)
    }

    fn deserialize_postgres_subsystem(
        system: SerializableSystem,
    ) -> Result<PostgresCoreSubsystem, ModelSerializationError> {
        let postgres_subsystem = system
            .subsystems
            .into_iter()
            .find(|subsystem| subsystem.id == "postgres");

        use core_plugin_shared::system_serializer::SystemSerializer;
        match postgres_subsystem {
            Some(subsystem) => {
                let postgres_core_subsystem = PostgresCoreSubsystem::deserialize(subsystem.core.0)?;
                Ok(postgres_core_subsystem)
            }
            None => Ok(PostgresCoreSubsystem::default()),
        }
    }

    async fn compute_spec(model: &str) -> DatabaseSpec {
        let postgres_core_subsystem =
            create_postgres_system_from_str(model, "test.exo".to_string())
                .await
                .unwrap();

        DatabaseSpec::from_database(&postgres_core_subsystem.database)
    }
}
